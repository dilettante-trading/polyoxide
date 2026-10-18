# Proposed spine amendments: Epic 8

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A8-1: the Kalshi product is `predictions` (amends the examples of AD-4 and AD-7)

- **Proposed by:** the owner, 2026-10-18, during Epic 8's design (spec D1, in `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`).
- **Current rule:**
  - AD-7 writes a Kalshi key as `kalshi.events:<ticker>:yes|no`, so the product is `events`.
  - AD-4's example of a canonicalized id is "Kalshi `<TICKER>:yes|no`".
- **Proposed rule:**
  - The Kalshi product is `predictions`, and AD-7's example is `kalshi.predictions:<ticker>:yes|no`. AD-4's example id, `<TICKER>:yes|no`, is unchanged.
  - The module is `polyoxide_kalshi::predictions`, its features are `predictions` and `predictions-ws`, and its suites are named `<kind>_predictions`.
  - `margin` stays free for Kalshi's perpetual futures.
- **Why:**
  - **Kalshi split its API.** It now has a Predictions exchange (`/trade-api/v2/…`) and a Margin exchange (`/trade-api/v2/margin/…`, perpetual futures).
  - **`events` collides.** It is also the name of Kalshi's own `/events` group and of every `Event` type, so a module named `events` reads as the wrong thing in both.
- **Follow-up in the code:** Story 1 declares the venue `kalshi` and the product `predictions` in `polyoxide-kalshi`'s `[package.metadata.polyoxide]` and writes `predictions::key`. Story 7 renamed the trading mapping's keys. The merge session edits AD-7's example and the guide's matching text.

## A8-2: the recipe's `RetryPolicy` is optional when core's default fits (amends the guide's step 3 and AD-17)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec D6).
- **Current rule:** step 3 of "Adding a venue" reads "Implement the venue's `Throttle` (compose core primitives), `RetryPolicy` and `Authenticator`". AD-17 says the loop retries only the venue policy's set.
- **Proposed rule:** a venue implements a `Throttle` and an `Authenticator`, and a `RetryPolicy` only when the statuses it retries or the hold it applies differ from core's `DefaultRetryPolicy`, which retries a 429 and holds every request for `RetryConfig::retry_delay(0)` and fails every other status, so it retries no 5xx, 408 or 425.
- **Why:**
  - **Kalshi's set is core's.** Kalshi's documentation says it sends no `Retry-After` and promises no cooldown, and gives a 429 the body `{"error":"too many requests"}`; a 429 was not provoked (`docs/specs/kalshi/OBSERVED.md`), so the default's floor is the whole policy.
  - **A copy would be a second definition.** A venue policy that restated the default would be a duplicate of it, which the duplication inventory exists to prevent.
- **Follow-up in the code:** Story 1 runs `Predictions` on core's `DefaultRetryPolicy`.

## A8-3: `CandleInterval` promises every width, so a venue that serves some cannot implement `Candles` (amends AD-6)

- **Proposed by:** the owner, 2026-10-18, who chose which traits Epic 8 implements (spec D10).
- **Current rule (AD-6):** optional abilities are separate traits, implemented only where offered, and no method returns `Unsupported`. `CandleInterval` names one minute to one week, each width every candle venue serves.
- **Proposed rule:** this is a proposal for the merge session to decide between. Either `Candles` lets a venue declare the `CandleInterval`s it serves, so a caller can ask before it calls, or a venue refuses a width it lacks as `InvalidRequest`, sent nowhere, as a key of another product is refused.
- **Why:**
  - **Kalshi serves three widths.** Its candlesticks come at 1, 60 and 1440 minutes.
  - **No implementation fits the rule as written.** The trait promises every width, and AD-6 forbids a method that answers "unsupported", so a venue with three widths can implement neither the whole trait nor a part of it.
- **Follow-up in the code:** Epic 8 leaves `Candles` unimplemented for Kalshi, and `deferred-work.md` holds the decision and the implementation.

## A8-4: the scripted test server records each handshake's request headers (amends AD-13's touch list and the guide's step 8)

- **Proposed by:** the owner, 2026-10-18, during Epic 8's design (spec D17).
- **Current rule:** a new venue touches only the files AD-13 lists, and Epic 8's own acceptance criteria add "no edit to `polyoxide-{venue,core,ws,test-support}`".
- **Proposed rule:** the list gains one additive, venue-neutral edit: the kit's scripted server, `polyoxide_ws::test_server`, which is doc-hidden and behind `test-server`, records each accepted handshake's request headers (`ScriptedServer::handshake_headers()`: names lowercase, in the order handshakes completed; a refused upgrade takes no slot, so an index is not a connection's index). It names no venue.
- **Why:**
  - **Kalshi signs its handshake on every attempt.** `PredictionsProtocol::handshake` signs `timestamp_ms + "GET" + <path>` afresh, with the signer REST uses.
  - **The server showed no headers.** It recorded paths and frames, so a suite could not show that a reconnect signs again instead of reusing the first signature, which is what a bug would look like.
- **Follow-up in the code:** Story 5 adds the recorder and its test, and edits CLAUDE.md's scripted-server paragraph (AD-21). The recorder changes three test-context items of the scripted server (`struct Recorder`, `fn handshake` and `fn serve` in `polyoxide-ws/src/test_server/mod.rs`), so it takes DRIFT R15 in `divergences.md`, three `[[drift]]` entries in `ci/test-body-diff.toml`, and the matching update of `.github/scripts/tests/test_test_body_diff.py`; the touch set admits both files (spec D20). Story 6's supervision suite proves a fresh signature on every attempt through it. It is additive, so `docs/s3-removals.md` gains nothing.

## A8-5: a new venue's header pins, and the umbrella's full-pin rule (amends AD-22 and the guide's step 8)

- **Proposed by:** the owner, 2026-10-18, who approved the CI edits (spec D20) and ran the flow profile's matching change (spec D24).
- **Current rule:**
  - CI derives a header pin, `tests/headers_<module>.rs`, for every venue crate that builds core, and the features job's two header-pin steps list each one: the minimal build, and the umbrella's `full`.
  - `test_the_features_job_runs_each_header_pin_under_the_umbrellas_full` asserts that the umbrella's `full` turns each such module on.
  - The recipe's list of what a new venue touches names no CI file.
- **Proposed rule:**
  - A new venue adds its pin to both steps (`cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`, and `cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`).
  - The "`full` turns each module on" assertion applies only to the venue crates the umbrella depends on, read from the umbrella's `dependencies` in the same metadata. A `publish = false` venue stays outside the umbrella and still runs its pin under `full`.
  - The flow profile's final `features` gate runs the same two commands, each guarded by `[ ! -d polyoxide-kalshi ] ||`, since every worktree shares the profile.
- **Why:** a published umbrella cannot depend on a `publish = false` crate, so the assertion could never hold for Kalshi, and without the two lines Kalshi's headers go unpinned in the one build that unifies every other venue's features.
- **Follow-up in the code:** Story 1 edits `ci.yml` and `test_ci_workflow.py`, with a new test that pins the rule both ways on stand-in metadata.

## A8-6: hand-written registration points the recipe does not name (amends AD-13's "A new venue touches only" and the guide's step 8)

- **Proposed by:** the owner, 2026-10-18 (spec D20 and D23).
- **Current rule:** AD-13 says registration is Cargo metadata, and lists what a new venue touches: its `members` line, its pins, its crate, its specs, its capture scripts, its allowlist files, its amendments file, `_bmad-output/specs/**` and regenerated regions.
- **Proposed rule:** the list also names the places a venue is registered by hand, or each one is derived from metadata:
  - `test_dep_allowlists.py`'s `PER_FEATURE` rule for the crate and its `MUST_BUILD` rows for the REST file and the socket file, since a crate with no `PER_FEATURE` entry is checked with its default features against the socket-only deny-list, which refuses `reqwest` and `polyoxide-core`, so any venue with an HTTP module fails it;
  - CLAUDE.md's hand-written paragraphs that list the venue crates, the header pins, the mock and live suites and the allowlist rules.
- **Why:** Kalshi's `PER_FEATURE` rule is `alloy*`, `hmac` and `keyring` never admitted, `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `predictions-ws`, and `reqwest`, `polyoxide-core`, `governor` and `sha2` with `predictions`, which every file names. Two `MUST_BUILD` rows keep the rules from passing vacuously: `"polyoxide-kalshi@predictions.txt": "reqwest"` (added in Story 1's review) requires the HTTP stack in the REST file, and `"polyoxide-kalshi@predictions-ws.txt": "polyoxide-ws"` requires `polyoxide-ws` in the socket file.
- **Follow-up in the code:** Story 1 adds the `PER_FEATURE` rule and edits the CLAUDE.md paragraphs it names. Story 6 adds the `@predictions-ws` `MUST_BUILD` row and edits the paragraphs on live suites and sockets.

## A8-7: where "on first use" happens (amends the guide's step 3 and AD-10)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec D5).
- **Current rule:** step 3 says that when a venue serves its limits or per-route costs from an endpoint, they are read on first use, with a drift test of the route table against the costs endpoint.
- **Proposed rule:**
  - The read runs in the client, before its first send, not in the throttle: one confirmation, run once by a `tokio::sync::OnceCell` that every route builder's `send` waits on, `health().ping()` included.
  - It reads the costs endpoint and replaces each compiled cost it lists. With credentials it then reads the limits endpoint and resizes the bucket in place (`CapacityBucket::resize`). It always ends confirmed: a failed read or a zero limit keeps the compiled costs or the provisional sizing, with one warning.
  - Costs are computed when a request is sent, after the confirmation, not when its builder is made.
- **Why:** a `Throttle` is a hook inside core's send loop, so one that sent a request would re-enter the loop. The step said "on first use" without saying where, and the throttle was the wrong place.
- **Follow-up in the code:** Story 1 writes the `before_send` stub and Story 4 replaces it with the confirmation. Story 2's `live_predictions` holds the drift test of the route table against `/account/endpoint_costs`.

## A8-8: the umbrella's generic example cannot be included without every venue's features (amends A6-2)

- **Proposed by:** the owner, 2026-10-18 (spec D22).
- **Current rule (A6-2):** the guide links `polyoxide/examples/venue_neutral.rs`, which reads the best bid and ask of each venue with one `best_bid_ask(&DynMarketData, &MarketKey)` and their errors with one classifier generic over `Classify`, and says "A new venue whose products implement the traits needs no change to it."
- **Proposed rule:** the generic half moves into a file of its own that names no venue, which the umbrella's example includes by `#[path]` and a venue's suite can include with no feature of another venue.
- **Why:** the example imports Polymarket's and Binance's types and has a `main` that uses them. Kalshi's suite includes it by `#[path]`, so its dev-dependency on `polyoxide` (with `default-features = false`) turns on the example's `required-features`, `polymarket-clob`, `polymarket-perps` and `binance-usdm`, and its tokio dev-dependency has `macros` and `rt-multi-thread`. That builds the Polymarket and Binance stacks into Kalshi's test builds. They are dev-only, so no `cargo tree -e normal` and no allowlist changes. The same unification means Kalshi's own "minimal build" header pin (`cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`) is not minimal from Story 3 on: it carries the other venues' features, which the umbrella-full pin exists to unify. It still passes, since core alone declares reqwest's features. The proposed split would also restore the minimal pin, because the generic half would need no dev-dependency on the umbrella's example.
- **Follow-up in the code:** Story 3 carries the dev-dependency and the include. The umbrella's files are not edited by Epic 8, and the merge session decides the split.

## A8-9: test-support's agreement and OpenAPI helpers read JSON only (amends the guide's step 6)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec D19).
- **Current rule:** step 6 says to add `wire_agreement_*` and `spec_agreement_*` tests using the test-support helpers.
- **Proposed rule:** a venue that publishes its spec as YAML reads it through a dev-only YAML parser in its own suite, or `polyoxide-test-support` gains a YAML reader behind a feature. The merge session decides between them.
- **Why:** Kalshi publishes no JSON (`openapi.json` and `asyncapi.json` are 404), and no YAML crate was in the tree, so Epic 8 added `serde-saphyr` as a dev-dependency of `polyoxide-kalshi` alone. A dev-dependency is in no `cargo tree -e normal`, so `ci/dep-allowlists/` is unchanged by it. Nor can test-support's `openapi::check` and `query::documented_parameters` read Kalshi's OpenAPI as served, so Story 2's spec-agreement suite normalises its own copy: it inlines `$ref` parameters, collapses single-arm `allOf` and gives property-less objects empty properties.
- **Follow-up in the code:** Story 2 adds the pin to the root `Cargo.toml` and the dev-dependency, and its `spec_agreement_predictions.rs` reads `docs/specs/kalshi/openapi.yaml` through it.

## A8-10: cargo-hack's `--ignore-private` leaves a `publish = false` venue's features unchecked (amends AD-22)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec §9.2).
- **Current rule (AD-22):** `cargo hack check --each-feature --no-dev-deps` runs on every venue crate. CI's features job runs it as `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`.
- **Proposed rule:** the features job also runs `cargo hack check -p polyoxide-kalshi --each-feature --no-dev-deps`, or `--ignore-private` is dropped once no private member is left that is not a venue.
- **Why:** `--ignore-private` skips every `publish = false` member, so `polyoxide-kalshi`'s features, `predictions` and `predictions-ws`, are not checked one at a time on a pull request. A feature that compiles only beside another would pass unseen. Epic 8 covers the common case with Kalshi's header pin, which builds `predictions` alone, and the nightly live jobs, which build each live target with its declared features alone; neither is `--each-feature`.
- **Follow-up in the code:** none in Epic 8: the cargo-hack step is outside its touch set (spec D20). `deferred-work.md` holds it.

## A8-11: a venue whose replay can be refused must stop on the refusal itself (amends the guide's step 4 and AD-15)

- **Proposed by:** the run, 2026-10-18 (Story 6 review); fix chosen by the owner (spec D16, amended).
- **Current rule:** the guide's step 4 says only "For sockets, implement a `Protocol`. Use `impl_ws_classification!` for socket errors." AD-15 says whether to reconnect or stop "is decided by the `Protocol`'s recovery hook, which by default reconnects if and only if `is_retriable()`", and AD-11 lists "venue-specific recovery" among what a venue describes. Neither says what the kit does with a `Recovery::Skip` returned for an error during an attempt (the connect or the replay): `polyoxide-ws/src/supervisor/mod.rs` and `task.rs` treat it as a reconnect, the same arm as `Recovery::Reconnect`.
- **Proposed rule:** the guide's step 4 and AD-15 say so, and say that a venue whose server can refuse the replay's subscription (Kalshi refuses a command naming a ticker it does not list) must stop on that refusal, or on an answer to the replay that does not decode, since either one, skipped, reconnects for ever.
- **Why:**
  - **Kalshi refuses a subscription that names a bad ticker.** A change made during an outage is answered at once and replayed (D13), so a bad ticker reaches the venue only on the replay.
  - **An answer to the replay that does not decode was skipped the same way.** The replay's subscribe got an answer the client could not read, D16 skipped it, and the stream looped as the refusal did.
  - **Skip during an attempt is a reconnect.** D16 skipped every error code it did not name, so the refused replay reconnected, was refused again, and the stream yielded nothing after its `Disconnected`.
- **Follow-up in the code:** Story 6's review made a replay refused with a code that does not reconnect end the stream with that error (a private flag that `recover` reads), and made codes 17 and 18 reconnect. The supervision suite pins both paths, a change made during an outage and one whose connection died under it. The final review round makes a replay answer that does not decode stop the stream too (spec D16, amended), pinned by a supervision test. The kit is unchanged.

## Audit of the epic's diff (spec §9.1)

Run over the range from `git merge-base main HEAD` to `HEAD`, by Story 7.

- **The touch set.** `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh` printed no path and exited 0.
- **Generated regions.** Every changed hunk of `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml` and `.github/workflows/nightly-schema.yml` lies inside a `generated:begin` and `generated:end` region.
- **The foundation.** The only files changed in `polyoxide-venue`, `polyoxide-core`, `polyoxide-ws` and `polyoxide-test-support` are `polyoxide-ws/src/test_server/mod.rs` and `polyoxide-ws/src/test_server/tests.rs` (Story 5, A8-4). Their `git diff --stat` is below.
- **No copies.** `polyoxide-kalshi` and its capture scripts define none of the helpers named in `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md`: a grep for each name defined locally (`fn <name>`, `struct <name>` and the other item keywords) found none.
- **`OBSERVED.md`.** Every probe and capture finding of spec §3.6, §4.4, §6.7 and §8.12 is recorded in `docs/specs/kalshi/OBSERVED.md`.

```
 polyoxide-ws/src/test_server/mod.rs   | 36 ++++++++++++++--
 polyoxide-ws/src/test_server/tests.rs | 79 +++++++++++++++++++++++++++++++++++
 2 files changed, 111 insertions(+), 4 deletions(-)
```
