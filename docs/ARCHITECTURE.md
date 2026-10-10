> **Copied from the spine's guide; do not edit.** The `architecture-guide` region below is
> `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-GUIDE.md`,
> byte for byte, and CI fails when the two differ. This file is only ever regenerated from
> the architecture spine (AD-21): amend the spine, regenerate its guide, then run
> `python3 scripts/gen_registry.py --write`, which copies the guide here and writes the stage
> line from `[workspace.metadata.polyoxide] stage` in the root `Cargo.toml`.

<!-- generated:begin architecture-stage -->
**The workspace is in stage S1.** [Which stage the workspace is in](#which-stage-the-workspace-is-in-ad-16) says what each stage changes.
<!-- generated:end architecture-stage -->

<!-- generated:begin architecture-guide -->
# polyoxide architecture guide

This is the agent's guide to the multi-venue polyoxide workspace. CLAUDE.md points here.

The guide is generated from the architecture spine,
`_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md`.
Rules are cited by spine id (`AD-n`), so you can check the exact wording there.

Precedence:
- **Spine and guide:** where they disagree, the spine wins, until this guide is regenerated from it.
- **Guide and CLAUDE.md:** where they disagree, this guide wins. The CLAUDE.md rules the restructure supersedes are listed in [Standing rules this replaces](#standing-rules-this-replaces-ad-21).

## The shape

polyoxide is a set of **venue crates**: Polymarket, Binance and later Kalshi. Each venue crate is
split into **modules**, one per host or product (`clob`, `gamma`, `data`, `relay`, `perps`, `rtds`,
`sports`, `usdm`, …). The venue crates sit on two **kernels**, one for HTTP and one for sockets,
and on one **vocabulary** crate that the HTTP kernel also depends on. Facades (the umbrella crate,
the CLI and the Python bindings) compose venues and hold no venue logic.

The rows below are layers. The edges are listed after them.

```text
facades      polyoxide · polyoxide-cli · polyoxide-py
venues       polyoxide-polymarket · polyoxide-binance · polyoxide-kalshi (planned)
kernels      polyoxide-core: HttpClient, the send loop, Throttle / RetryPolicy / Authenticator,
                             the hold, WindowQuotaTable, capacity bucket, keychain
             polyoxide-ws:   TLS, Backoff, connect, the bare tier, Supervisor + Protocol, test_server
vocabulary   polyoxide-venue: MarketKey / RawKey, Class / ClassifiedError, records, Extensions,
                              traits, Secret, UnixMillis, wire-enum macros
dev-only     polyoxide-test-support: agreement helpers, fixtures, soak harness, or_fail reporter
```

The allowed edges (AD-1):
- A venue crate depends on `polyoxide-core`, `polyoxide-ws` and `polyoxide-venue`, and on nothing else in the workspace.
- `polyoxide-core` depends on `polyoxide-venue`.
- `polyoxide-ws` depends on neither. That fence keeps the credential-free feeds (rtds and sports) free of HTTP and signing code (AD-2).
- Facades may depend on venue crates, `polyoxide-core` and `polyoxide-venue`.
- `polyoxide-test-support` depends on `polyoxide-core` and `polyoxide-venue`, and is only ever a dev-dependency.

### Which stage the workspace is in (AD-16)

The picture above and the routing table below describe the **end state**. The restructure
reaches it in three releases:
- **S1, internals:** the new crates and shared code. Crate names do not change yet.
- **S2, renames:** crates merge into `polyoxide-polymarket`, and the CLI and Python move to the venue layout.
- **S3 and later, additions:** traits, a supervised clob socket, and the Kalshi skeleton.

Before S2, a path such as `polyoxide-polymarket/src/clob/` still means `polyoxide-clob/src/`. The
full stage table is under [Release stages](#release-stages-ad-16-ad-25).

### How a request flows (AD-8)

Every HTTP request in every venue crate runs through `polyoxide-core`'s one send loop. Each
attempt runs these steps in order:
1. acquire a concurrency permit;
2. `throttle.acquire(meta)`, which charges the request's `costs` (the `&[Cost]` its request builder computed);
3. `authenticator.sign(parts, attempt)`, on every attempt, so timestamps stay fresh;
4. send;
5. `throttle.observe(...)`, on every response, including the last attempt's;
6. `policy.decide`, which returns `Done`, `Retry(wait)` or `Fail`, plus an optional **hold**. A hold is a wait every request on the throttle must serve; today's code calls it a cooldown.
7. `throttle.hold(h)`, if the decision carries a hold;
8. sleep for the longest of the loop's own backoff, `Retry-After` and the policy's wait.

## Where does my change go?

| You are adding or changing… | Put it in | Rule |
| --- | --- | --- |
| A new route on an existing host | `polyoxide-<venue>/src/<module>/api.rs` (or `…/api/`), as a request builder that computes its `costs` from the module's route table | AD-8, AD-10 |
| A rate limit, ban or tier rule | the venue's composed `Throttle` (in `src/shared/` or `src/<module>/`), built on core's `WindowQuotaTable` or capacity bucket. Never write your own retry loop | AD-8, AD-10, AD-23 |
| Which statuses get retried | the venue's `RetryPolicy`. Polymarket has one, shared by all its modules. No policy retries 5xx or 408 | AD-17 |
| Request signing | the venue's `Authenticator` | AD-8 |
| A socket feed that sends subscriptions or pings | a `Protocol` for `polyoxide-ws`'s `Supervisor`. Never a hand-written supervision loop | AD-11 |
| A new error | a variant of the module's `<Module>Error` (or `<Module>WsError` for a module with a socket), classified by the module's single decode function. See [Error classification](#error-classification-ad-15) | AD-15, AD-5 |
| A type every venue shares (a record, key, id, enum macro or time type) | `polyoxide-venue` | AD-3 |
| A field only one venue has, on a normalized record | the owning product's `<Record>Ext` in `Extensions`. Never a new field on the normalized record | AD-19 |
| A test helper used by several crates | `polyoxide-test-support`. If inline unit tests need it, keep it in the crate under test or in `polyoxide-ws`'s `test-server` instead | AD-1 |
| A new crate, live test, spec mirror or README table row | the crate's `[package.metadata.polyoxide]`, or `[workspace.metadata.polyoxide.mirrors]` for a mirror no crate covers. Then run `scripts/gen_registry.py`. Never hand-edit a generated region (text between `<!-- generated:begin <id> -->` and `<!-- generated:end <id> -->`) | AD-13 |
| A CLI command | `polyoxide <venue> <module> <verb…>`, using the shared runner and `OutputFormat` | spine §Consistency Conventions |

## Rules that bite if you miss them

### Request path

- **Waits only lengthen (AD-9).** A policy may lengthen a wait but never shorten it.
- **Only a 429 or a venue ban sets a hold (AD-9).** A 425 retry waits for that request alone. Every 429 sets a hold, even with no retry left.
- **Holds are throttle state, not client state (AD-23).** Binance's weight budget is shared by every client on one IP, so a ban on one client must stop all of them.
- **Three bucket models, kept apart (AD-10):**
  - A published *window quota* (Cloudflare) is depth 1 with a tenth reserved.
  - A published *capacity* (Polymarket's per-signer buckets, Kalshi's token buckets) uses `allow_burst(capacity)`.
  - Binance's *weight minute* is a UTC-minute counter that stays in `polyoxide-binance`.

  Making them "consistent" brings back a measured 2x over-permit.
- **The retry log line is load-bearing (AD-8).** The retry `WARN` line `Retriable status …`, emitted under tracing target `polyoxide_core`, is what the soak harnesses count throttling by.

### Error classification (AD-15)

Each module's decode function maps an error into one of eight `Class` variants:

| `Class` | When |
| --- | --- |
| `Network` | no response: connect, timeout, reset, TLS EOF, DNS |
| `Unavailable` | 408, 425, 5xx |
| `RateLimited` | 429 |
| `Unauthorized` | 401, 403 |
| `Restricted` | 418, 451 |
| `VenueRefusal` | any other 4xx |
| `InvalidRequest` | a client-side refusal or misuse (`Refused`, an undeclared extension, local validation, a bad URL, TLS name or handshake format, use after close); never from a status |
| `Decode` | a 2xx body that does not parse |

- The status decides the class before the body does.
- A venue overrides a status only through a DFR row (see [Consolidating code](#consolidating-code-ad-12)). Binance's 403 becomes `Restricted` this way.
- Venue traits return `ClassifiedError`, never the module enum. Typed module errors stay on the native clients.

### Types and keys

- **Trait shape (AD-5).** The dyn-held traits declare `Send + Sync` supertraits, and never a `'static` supertrait, which dynosaur rejects. These traits are `Throttle`, `RetryPolicy`, `Authenticator`, `MarketData`, the capability traits (`Trades`, `Candles`, `Funding` and `EventGroups`) and `Trading`.
  - A method that returns a stream returns `impl Stream<Item = …> + Send + Unpin + 'static`.
  - Use `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`.
- **Keys are canonical (AD-4).** Build a `MarketKey` only through its product's constructors, for example `polyoxide_binance::usdm::key(...)`. Parse text with `RawKey::parse`, then pass the `RawKey` to `polyoxide::parse_key` or to the product's constructor.
- **On event-contract venues (Polymarket's clob, Kalshi), a key names an outcome, not a market (AD-7).**

### Sockets and trading

- **Supervision invariants (AD-11):**
  - per outage, at most one `Disconnected` (only if your `Protocol` declares it) and exactly one `Reconnected`;
  - pings keep running while the consumer is slow;
  - handshake auth is recomputed on every attempt;
  - membership is yours to define, through `Protocol::Membership` and `Protocol::wanted`.

  The clob user channel's membership is `Option<Vec<Market>>`. `None` means every market, and an empty `Some` never becomes `None`.
- **Fills are never dropped (AD-24).** The trading event stream is unbounded. After a reconnect on a fills channel, it yields `TradingEvent::Resync`, and the consumer reconciles.
- **Kill outcomes are not faults (AD-20).** Through `Trading`, a FAK or FOK kill is `Ok` with status `Killed`.

### Releases (AD-25)

- Never bump the version on a loom or integration branch.
- The version bump is a separate commit on `main`. Make it after `git fetch` and after checking that the new version is not already on crates.io.
- Releases publish only crate versions not yet on crates.io, so a failed release can resume.
- `release.yml` runs only for a push to `main` in this repository, never for a fork's branch that happens to be called `main`.
- A release whose changes include a removal must raise the 0.x minor version.

## Consolidating code (AD-12)

Before merging two similar-looking pieces of code, check the DFR rows in
`_bmad-output/specs/spec-venue-extensibility/divergences.md`. A DFR row is a divergence kept for a
stated reason. If a difference changes behaviour, it stays at the venue's call site. A DRIFT row is
a difference with no recorded reason, and the same file records its decided fix.

When you move or consolidate tests:
- Every moved test keeps its function name and its assertions.
- A consolidated helper must cover every case that each copy it replaces covered.
- A PR that moves tests reports per-suite counts before and after.

The suites that matter most:
- supervision (perps, usdm, rtds, sports);
- `documented_*_limits`, which assert *effective* quota, not mere presence;
- the limiter, cooldown and `Retry-After` mutation tests;
- `classify_order_kill`;
- the EIP-712 and session-key golden vectors;
- spec and wire agreement, with their allow-lists;
- the live drift detectors;
- the nightly classifier's tests.

The mutation-tested rules and their mutants are listed in `docs/MUTANTS.md`. They are kept there,
not in this guide, because this guide is regenerated.

## Live tests and the nightly run (AD-14)

- Unwrap each venue call's `Result` with `.or_fail("context")` (`ResultExt` in `polyoxide-test-support`), not `.unwrap()`. `or_fail` prints `polyoxide-class=<tag>`, and the nightly classifier reads only these tag lines.
- Load credentials through the credential loaders in `polyoxide-test-support`. When credentials are absent or empty, the loaders print `auth-gated`, and the nightly run skips the test silently. An unset repository secret arrives as an empty string. The env names you pass must equal your target's declared `secrets`, which CI checks.
- Call `environmental(reason)` when the world, not the code, is the reason a test cannot run.
- Call `transient(reason)` when a stream ends without a close code. That case used to be caught by the "server ended the connection" regex.
- Every test in `tests/live_*.rs` is `#[ignore]`. Each `tests/live_*.rs` target has a `live.<target>` entry in the crate's `[package.metadata.polyoxide]`, and that entry declares the target's `secrets`. The nightly workflow's secret wiring is generated from those declarations, so never edit it by hand.

## Adding a venue

The Kalshi walking skeleton is this recipe's first user. Record every step the recipe misses in
`spine-amendments/<epic>.md` beside the spine. One session merges these into the spine, and this guide
is then regenerated from the spine.

1. Create `polyoxide-<venue>/`. Add one `members` line and one `[workspace.dependencies]` pin. Declare the venue and product ids in `[package.metadata.polyoxide]`, and add a unit test asserting that the crate's `const`s equal them.
2. Lay modules out as `src/<module>/{api,ws,types,error,venue}`. Put anything shared across modules in `src/shared/`, gated by the features that use it.
3. Implement the venue's `Throttle` (compose core primitives), `RetryPolicy` and `Authenticator`. Each request builder computes its `costs` from the route table.
   - Measure window quotas with a soak over distinct URLs, and record the results in `docs/specs/<venue>/OBSERVED.md`.
   - A capacity the venue publishes, or serves from a limits endpoint, may be used provisionally. Record it as provisional in `OBSERVED.md` until a soak confirms it; never copy a limit from prose docs without recording it.
   - When the venue serves its limits or per-route costs from an endpoint, read them on first use, and add a drift test of your route table against the costs endpoint. Core's capacity bucket resizes in place, so this needs no core edit.
4. For sockets, implement a `Protocol`. Use `impl_ws_classification!` for socket errors.
5. Write one `<Module>Error` per module (plus `<Module>WsError` for a module with a socket), with a single decode function that maps into `Class`.
6. Set up the drift-detector pattern:
   - mirror the venue's published spec in `docs/specs/<venue>/` with a `mirrors` entry, or record in `OBSERVED.md` that it has none;
   - write `OBSERVED.md`;
   - capture fixtures in `tests/fixtures/<module>/`, with `PROVENANCE.md`, using `scripts/capture_<venue>_<module>.py`;
   - add `wire_agreement_*` and `spec_agreement_*` tests, using the test-support helpers;
   - add a `live_*` test with a no-unmodelled-keys check.
7. Implement the `polyoxide-venue` traits the venue supports, in `venue.rs`: `MarketData`, then any capability traits, then `Trading`. Put venue-only fields in `<Record>Ext`.
8. Run `scripts/gen_registry.py`. Touch nothing outside these (AD-13):
   - your `members` line, your `[workspace.dependencies]` entries (new third-party pins included) and `Cargo.lock`;
   - your crate, including its `gate_exceptions`;
   - `docs/specs/<venue>/` and your mirrors entry;
   - `scripts/capture_<venue>_*.py`, built on `scripts/capture_common.py` with a header function for any signing;
   - `ci/dep-allowlists/<crate>*.txt`;
   - your `spine-amendments/<epic>.md`;
   - `_bmad-output/specs/**` records;
   - the generated regions.

## Release stages (AD-16, AD-25)

| Stage | What lands | What breaks for consumers |
| --- | --- | --- |
| S1, internals | the new crates (`polyoxide-venue`, `polyoxide-ws`, `polyoxide-test-support`), the one send loop, `Supervisor` (perps and usdm migrated), DRIFT fixes, registration (AD-13), CI, the CLI on crates.io | Consolidated items' old paths (a checked-in removal list, gated by `cargo semver-checks`); error and builder shapes |
| S2, renames | `polyoxide-polymarket`, the multi-venue umbrella, the CLI venue tree, Python modules, feature renames; built in one loom integration session with a draft PR to `main` | Every remaining path. The rename manifest is the migration guide, and retired crates get one tombstone release |
| S3 and later, additions | traits, a supervised clob socket (a new type), the Kalshi skeleton (`publish = false`) | nothing |

## Standing rules this replaces (AD-21)

Edit each rule's CLAUDE.md text in the commit that supersedes it. Where that text is inside a
generated region, change it only by running `scripts/gen_registry.py`.

| Superseded CLAUDE.md rule | Replaced by |
| --- | --- |
| rtds and sports depend on nothing in-workspace | AD-2 and [The shape](#the-shape): they depend on `polyoxide-ws` and `polyoxide-venue` |
| per-crate `ensure_crypto_provider` copies | AD-11: one copy in `polyoxide-ws` |
| the hand-written publishing order and crate graph | AD-13: generated from Cargo metadata |
| `AUTH_GATED_RE` instructions | AD-14: `auth-gated` tags from the credential loaders |
| "call `note_rate_limited` before `should_retry`", and `should_retry`'s 429/425 set | AD-8, AD-9, AD-17: [How a request flows](#how-a-request-flows-ad-8) |
| `impl_api_error_conversions!` | AD-15: per-module decode functions |
| the `live_api.rs` and `mock_api.rs` naming, and Module Organization | spine §Consistency Conventions |
| "Binance not in the umbrella", clob's `ws` feature and the `polyoxide.v2` namespace | AD-16, S2: the multi-venue umbrella, `clob-ws` and `polyoxide.polymarket.data.v2` |
<!-- generated:end architecture-guide -->
