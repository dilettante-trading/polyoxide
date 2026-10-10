# Epic 4 Context: One socket kit and supervisor, and the S1 release

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

This is stage S1. The plan has Epic 4 starting after Story 2.6 and running in parallel with Epic 3. In practice Epics 1–3 are done and merged to `main` (cb476e2) at version 0.38.2, unreleased, so Epic 4 is the last S1 work.

Today the socket code is copied across five crates (clob, perps, Binance, rtds, sports):
- Each crate has its own TLS install, backoff, error wrapper, bare-tier `poll_next` and scripted test server.
- perps and Binance each run their own supervision task.
- rtds and perps have no connect timeout.
- rtds treats a 503 or a 429 at reconnect as fatal.
- Only sports sends the close reply.

This epic adds `polyoxide-ws`, a socket kit with a generic `Supervisor<P: Protocol>` and one scripted test server. It moves perps and Binance onto the `Supervisor` with their suites unchanged, and puts rtds, sports and clob on the kit's blocks. Every socket error goes through one classification table, and only `polyoxide-ws` depends on tokio-tungstenite and rustls. The credential-free feeds still build no HTTP or signing code. The epic ends by cutting S1 as one release with a complete migration list for prader-rs.

## Stories

- Story 4.1: The socket kit crate
- Story 4.2: Connect, classify and close, once
- Story 4.3: One socket-error classification table
- Story 4.4: One scripted test server
- Story 4.5: The generic Supervisor
- Story 4.6: perps on the Supervisor
- Story 4.7: Binance on the Supervisor
- Story 4.8: rtds and sports on the kit
- Story 4.9: clob's sockets on the kit
- Story 4.10: Socket error enums reshaped, and socket transport ownership
- Story 4.11: Cut the S1 release

## Requirements & Constraints

- **Behaviour is preserved unless a DRIFT row says otherwise.** Each DRIFT row lands as its own commit, named for its row.
  - **R1.** A refused handshake with status 408, 425, 429 or 5xx is retried on every venue, matching core's HTTP rule. perps adds 408 and 425. rtds stops treating a 503 or a 429 at reconnect as fatal.
  - **R2.** Every socket connect has a timeout, 10 s by default, through the shared connect-with-timeout. rtds and perps have none today.
  - **R3.** The bare tier sends the RFC 6455 close reply for every venue. Before fixing it, confirm on the wire against one live host that the reply is missing today, and note the capture in the PR.
  - **R9.** The live tests call the shared TLS function. Three of them call `install_default` themselves: sports `live_api`, Binance `live_ws` and clob `live_ws`.
  - R1 and R2 change behaviour users can see, so the S1 notes name them, along with Epic 3's R4.
  - R6, rtds' `test-fixtures` feature renamed to `test-server`, waits for S2.
- **DFR rows stay per venue**, at the venue's `Protocol` or call site. Only identical or parametric copies merge.
  - **D4, ping schedule.** rtds pings on a quiet tick; perps and Binance ping on the wall clock; sports sends no pings; clob pings every 10 s. Never copy the rtds pump shape to a host that requires pings.
  - **D5, liveness for staleness.** sports counts pings, rtds counts only decoded events, Binance counts any frame, and perps counts frames plus an ok pong.
  - **D6, backoff reset.** What counts as a delivered connection follows D5. It reaches `Backoff` as a `bool` from the call site.
  - **D7, supervision shape.** sports is a `Stream` state machine and rtds a `run(handler)` future. Both stay outside the `Supervisor`.
  - **D8, outage markers.** prader folds Binance's `Disconnected{path}`→`Reconnected{path}` pairs. perps has no `Disconnected`. rtds signals a reconnect with a new `Snapshot`.
  - **D9, Binance limits.** Rotate at 23 h 50 min. Replay paced at 200 names per request and one request per 200 ms, with the 1024-stream cap enforced first.
  - **D10, perps.** `Recovery::Retry` and `SequenceRegressed` stay perps-specific.
  - **D11, rtds.** Snapshot and backfill semantics, and the correction of the mislabelled spot snapshot.
  - **D12, clob.** clob decodes Binary frames as text.
  - **D13, two backoffs.** Core's jittered `RetryConfig::backoff` is never unified with the socket `Backoff`.
  - **D18, dependency weight.** rtds and sports build nothing heavy.
  - **D19, use after close.** After `AlreadyClosed`, perps and rtds stop, while Binance and sports reconnect.
- **Credential-free fence (AD-2, NFR2).**
  - **`polyoxide-ws`'s dependencies.** Only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls (`ring`, `std`), with serde and serde_json optional. It depends on neither `polyoxide-core` nor `polyoxide-venue`.
  - **Allowlist checks.** CI diffs `cargo tree -e normal` against a committed per-crate allowlist (`ci/dep-allowlists/<crate>*.txt`; neither the directory nor the check exists yet).
    - Story 4.1 adds the check for `-p polyoxide-ws`.
    - Story 4.8 adds it for `-p polyoxide-rtds` and `-p polyoxide-sports`.
  - **Socket-only deny-list.** `reqwest`, `alloy*`, `polyoxide-core`, `governor`, `hmac`, `sha2` and `keyring`.
  - **What a socket-only crate may build.** `polyoxide-ws`'s dependencies, `polyoxide-venue`'s, and its own. Venue's default build now depends on nothing (amendment A3-4), so the allowlists follow that, not the longer list AD-3 gives.
- **Test custody (AD-12, NFR7).**
  - **Protected supervision suites:**
    - perps inline: 21 (17 supervision, 3 backoff, 1 defaults);
    - Binance: `tests/supervision.rs` (21), `supervision_edges.rs` (7), the 3 tests inline in `usdm/ws/supervised.rs`, and the usdm ws client's inline tests;
    - rtds: `tests/supervision.rs` (4) plus 15 inline, 19 in all. Its 7 backoff tests become the canonical `Backoff` suite;
    - sports: `supervision.rs` (15), `bare.rs` (6) and 6 inline, 27 in all.
  - **Also protected:** Story 2.2's class tables, the `tests/failure_tags.rs` twins (Binance, sports, rtds), the ws wire agreement and the live drift detectors.
  - **How tests move.** A moved test keeps its name and assertions. Each moving PR reports per-suite counts before and after.
  - **Old loops.** An old supervision loop is deleted only once its suites pass against the `Supervisor`.
- **Removals.**
  - **No shims or re-exports.** Every removed public item goes in `docs/s1-removals.md`, which stays cumulative against `v0.38.1`. Removals expected in this epic:
    - perps' and Binance's `supervised` modules, `MembershipHandle` and `Recovery`;
    - each venue's `test_server` module.
  - **Doc-hidden paths.** The `test_server` paths are on that file's "Doc-hidden paths consumers import" list, which CI compiles. Removing one means marking its entry `**Removed**`, with the story and its replacement.
  - **What keeps its name.** Fixtures stay with their venues. rtds' `fixtures` and the `test-fixtures` feature keep their names until S2.
  - **Error enums.** Their names do not change until S2. Changed variants are listed for prader.
- **Standing rules (AD-21).** CLAUDE.md is edited in the change that supersedes each rule. Text inside generated regions changes only through `[package.metadata.polyoxide]` and `python3 scripts/gen_registry.py --write`.
  - **Story 4.9** removes the last per-crate `ensure_crypto_provider`, so it rewrites the copies text ("each with its own `ensure_crypto_provider` copy", rtds' "own copy", "WebSocket TLS needs a nudge").
  - **Workspace dependencies of rtds and sports.** "They depend on nothing else in the workspace" becomes "they depend on `polyoxide-ws` and `polyoxide-venue`". That line comes from each crate's metadata `notes`.
  - **Socket classification.** CLAUDE.md's "socket errors keep their inherent `is_retriable`/`retry_after` until Epic 4" and "per-crate `tungstenite::Error` match stays until Story 4.3" end with Stories 4.3 and 4.10.
- **Gates (NFR9).** A red job withholds the release:
  - clippy and rustdoc with `-D warnings` (a `pub` doc must not link a `pub(crate)` item);
  - MSRV 1.91;
  - `cargo hack` on each feature;
  - the publish dry run;
  - the removals job.

## Technical Decisions

- **What `polyoxide-ws` defines, once each:**
  - `ensure_crypto_provider` (W1);
  - `Backoff` (W3), with clamp, doubling, ceiling and reset;
  - the timing setters and defaults (W4);
  - the deadline arithmetic `min(ping, stale, age)` with saturation (W5);
  - the boxed `WsError` wrapper (W6);
  - connect-with-timeout, defaulting to 10 s (W8);
  - handshake-status and close classification blocks (W7, W9);
  - the bare-tier skeleton (W11). It skips Ping, Pong and Frame for callers that ask, and passes every frame to callers that need all of them. Its `close()` sends the R3 reply;
  - request/answer correlation (W12);
  - behind the one `test-server` feature: the scripted server (W13) and the helpers `next_event`, `fast` and ping counting (W15).
- **The status rule is injected.** `polyoxide-ws` cannot see `polyoxide-venue`, so its classification blocks take the retriable-status rule as a function. Every caller passes venue's, so the rule has one definition. Venue already exports `class_for_status`, `class_for_handshake_status` and `class_for_close_code`.
- **Socket error table (AD-15, Story 4.3).** `impl_ws_classification!` is a `macro_rules!` exported by `polyoxide-venue`. It expands in the venue crate against `::polyoxide_ws::WsError`.
  - **`Network`:** `Io` (a TLS EOF included), `Protocol`, `ConnectionClosed`, and close codes 1000, 1001, 1006 and 1011–1013.
  - **`VenueRefusal { code }`:** close codes 1002, 1003, 1007–1010 and 4000–4999.
  - **`InvalidRequest`:** `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity` and `AlreadyClosed`.
  - **A handshake `Http(status)`** follows the status rule.
  - **Folded in from Epic 2's deferred work:**
    - A refused upgrade's `Retry-After` is read into `RateLimited { retry_after }` for a 429 and `Restricted` for a 418.
    - clob's "no addresses resolved" DNS miss, built today as `InvalidMessage`, becomes `Network`.
    - A 451 handshake refusal is a non-fault everywhere. Binance already does this; rtds' `Server { status: 451 }` does not.
  - **Testing.** A table test in a crate that depends on both venue and ws covers every row.
- **Classification is separate from recovery.** Whether to reconnect or stop belongs to the `Protocol`'s recovery hook. By default it reconnects only when the class is retriable. Venue-specific recovery, such as perps' `Retry`, moves into its `Protocol`. A `Restricted` failure tags `environmental` when it is not a fault and `real` when it is (A2-1).
- **The `Supervisor` (AD-11, NFR3).** A venue implements only a `Protocol`:
  - **Handshake.** A handshake request built on every attempt, so its auth is recomputed each time. Kalshi signs a millisecond timestamp. Auth may go in the handshake or in the subscribe frame.
  - **Frames.** Decode, liveness and delivered predicates that see every frame type: Text, Binary, Ping, Pong and Close.
  - **Pings.** A ping schedule.
  - **Membership.** `P::Membership`, with paced `replay`, and `wanted`.
  - **Optional and venue-specific.** An optional `max_connection_age`, recovery and protocol-originated events, and whether it declares `Disconnected`.
  - **Queue.** Its queue, `Bounded(n)` or `Unbounded`.

  `Protocol` is generic-only, so ws needs no dynosaur.
- **The `Supervisor`'s invariants:**
  - **Outage markers.** Each outage yields at most one `Disconnected`, and only for a declaring `Protocol`. It yields exactly one `Reconnected`, even when membership empties mid-outage.
  - **Pings and silence.** Pings stay on the wall clock. While the queue is full, the `Supervisor` races `reserve()` against the ping timer and stops reading. Blocked time is not silence.
  - **No drops.** A decoded event is never dropped; a bounded queue backpressures.
  - **`wanted` turning false.**
    - On a healthy connection, it closes politely with no marker.
    - Mid-outage, a declaring `Protocol` first yields the pairing `Reconnected`.
  - **Routed paths.** A router ends a path only by emptying its membership.
  - **Logging.** It logs under the `polyoxide_ws` target. Libraries never install a subscriber.
- **Per-venue settings:**
  - **perps:**
    - pings with JSON `post/ping` on the wall clock, and an ok pong counts as liveness;
    - `message_rate_limited` means retry without reconnecting;
    - it emits `SequenceRegressed`;
    - it declares no `Disconnected`, stays wanted when its membership is empty, and uses a bounded queue.
  - **Binance:**
    - one `Supervisor` per routed path (`/market`, `/public`);
    - an empty path is not wanted;
    - connections rotate through `max_connection_age`;
    - replay is paced, and the stream cap is checked first (D9).
  - **rtds:** keeps `run(handler)` and uses kit blocks only: Backoff, connect-with-timeout, the handshake classifier, TLS, the close reply and the test-server core.
  - **sports:** keeps its `Stream` state machine, pings as liveness, and its unfixable-refusal rule (`retrying_can_fix`). It uses kit blocks only.
  - **clob:**
    - it adopts the kit's TLS function, `WsError`, bare tier and test server, and the shared server replaces its two ad-hoc loopback servers;
    - it keeps Binary-as-text and its 10 s `PING`;
    - `WebSocketWithPing` keeps its behaviour and public shape;
    - its per-address connect stays clob's own, wrapped by the shared timeout, and is recorded as a new DFR row through `bmad-spec`;
    - clob is not supervised in S1. Its supervised socket is a new type in Epic 7.
- **The scripted server (W13).**
  - **What it provides:**
    - an accept loop with one script per connection, where the last script repeats;
    - `connection_count` and `wait_for`;
    - reject, stall, refuse-status and close-code actions;
    - recorders for closes, pongs and frames;
    - the interval-timer idiom;
    - a pluggable `answer`.
  - **How it is built.** Only from published crates, never from `polyoxide-test-support` (AD-1).
  - **What each venue keeps.** Only its protocol `answer` and its fixtures.
- **Socket error reshape (Story 4.10).**
  - `PerpsWsError`, `UsdmWsError`, `SportsError`, `RtdsError` and clob's `WebSocketError` each invoke `impl_ws_classification!`.
  - Story 2.2's class tables give every case the same class. The only exceptions are R1's deliberate changes to rtds' and perps' handshake statuses, each named in its commit.
  - Binance's `Stopped` should keep its stopping cause. A stream stopped by a 451 refusal then tags `environmental` rather than `real` (deferred from Epic 2).
- **Transport ownership (AD-18).**
  - No venue crate depends directly on tokio-tungstenite or rustls.
  - `polyoxide-ws` re-exports the types that `Protocol`s need.
  - Story 3.13's fence exists: `.github/scripts/tests/test_dependency_fences.py`, which today covers reqwest and governor. Story 4.10 extends it rather than adding a standalone check.
- **Classify coverage.** `.github/scripts/tests/test_classify_coverage.py` sweeps every public error type under `polyoxide*/src` and requires a `Classify` impl. `polyoxide-ws` cannot implement `Classify` because it does not depend on venue. Its `WsError` therefore needs an exemption there, or it must be classified only through the venue enums that wrap it.
- **The S1 release (AD-16, AD-25, Story 4.11).**
  - **Version.** `main` carries Epics 1–3's removals at 0.38.2. `release.yml`'s semver job compares against `v0.38.2` and fails a patch bump on a major-level change, so the S1 bump raises the 0.x minor.
    - The bump is a separate commit on `main`, made after `git fetch` and a crates.io check, and it is the last commit before the tag.
    - No bump is ever made on a loom branch.
    - The removals baseline stays `v0.38.1` until S1 ships.
  - **New crate names.** Three are published for the first time: `polyoxide-venue`, `polyoxide-ws` and `polyoxide-cli`. The limit is five per release.
  - **Before the release.**
    - Confirm that the crates.io token covers `polyoxide*`, including publish-new for `polyoxide-cli` (404 today), and that it expires after S3.
    - The `cargo` and `pypi` environments' restriction to `main` is still unset. It is a repository setting the user changes, not the agent.
  - **Removal list.** `docs/s1-removals.md` is final and gives call-site changes beside each path.
  - **Release notes.** A consumer-impact section names DRIFT R1, R2 and R4 and the log targets moved to `polyoxide_ws`. `deferred-work.md` holds about 17 entries addressed to Story 4.11 that list Epics 1–3's consumer-visible changes; they are the notes' source.
  - **After the release.** A smoke check proves that `cargo install polyoxide-cli` works from crates.io.
  - **Stage line.** The guide's stage line is generated from `[workspace.metadata.polyoxide] stage` (currently `S1`) in the root `Cargo.toml`. Change it there and run `gen_registry.py --write`, as well as recording the stage in the spine.

## Cross-Story Dependencies

- **Prerequisites, all landed:**
  - Story 2.6 (socket live suites on the tag reporter);
  - Story 1.7 (the removal gate);
  - Stories 2.1 and 2.2 (`Classify`, the status and close-code maps, the socket `Classify` impls);
  - Story 3.13 (the dependency fence).
- **Within Epic 4:**
  - 4.1 comes first. Stories 4.2–4.4 build on its crate.
  - 4.3 needs 4.1's `WsError`.
  - 4.5's unit suite runs against 4.4's server, using a test `Protocol`.
  - 4.6 and 4.7 need 4.5, and they move their suites onto 4.4's server.
  - 4.8 and 4.9 need 4.1, 4.2 and 4.4.
  - 4.10 needs 4.3, plus the migrations (4.6–4.9): perps' `Retry` must already live in its `Protocol`, and no venue may still use tungstenite directly.
  - 4.11 needs Epics 1–4 complete and CI green on `main`.
- **Later epics:**
  - Epic 5 (S2, one loom integration session) moves the socket crates into `polyoxide-polymarket`. It also does R6, renames the error enums, and runs AD-2's check per feature.
  - Epic 7's supervised clob market and user sockets build on 4.5 and 4.9.
  - Epic 8's Kalshi socket uses the `Supervisor`, with per-attempt handshake auth.
- **Spine.**
  - Amendments A2-1 and A3-1 to A3-5 are accepted but not merged into the spine. They override it where the two differ.
  - Epic 4 records its own amendments in `spine-amendments/epic-4.md`, which does not exist yet.
  - No story is assigned to the merge session.
