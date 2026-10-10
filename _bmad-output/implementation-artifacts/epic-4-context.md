# Epic 4 Context: One socket kit and supervisor, and the S1 release

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

This is stage S1, and Epic 4 is its last work. Epics 1–3 are merged to `main` (cb476e2) and unreleased: `main` still reads 0.38.2, which was tagged (`v0.38.2`) before the merge. Stories 4.1–4.4 landed as bundle K on `aidanb/restructure-run`, ahead of `main` and at review; Stories 4.5–4.11 remain.

The socket code is still copied across five crates (clob, perps, Binance, rtds, sports), and stays so until each venue's story moves it:
- Each crate has its own TLS install, backoff, error wrapper, bare-tier `poll_next` and scripted test server.
- perps and Binance each run their own supervision task.
- rtds and perps have no connect timeout.
- rtds treats a 503 or a 429 at reconnect as fatal.
- Only sports sends the close reply.

Bundle K added three things, and no venue uses any of them yet:
- `polyoxide-ws`, the socket kit, with one copy of each block;
- `polyoxide_venue::impl_ws_classification!`, the one socket classification table;
- one scripted test server.

The rest of the epic does five things:
- adds a generic `Supervisor<P: Protocol>`;
- moves perps and Binance onto it with their suites unchanged;
- puts rtds, sports and clob on the kit's blocks;
- reshapes every socket error onto the one table;
- leaves `polyoxide-ws` as the only crate that depends on tokio-tungstenite and rustls.

The credential-free feeds still build no HTTP or signing code. The epic ends by cutting S1 as one release with a complete migration list for prader-rs.

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
  - **R1.** A refused handshake with status 408, 425, 429 or 5xx is retried on every venue, matching core's HTTP rule.
    - perps adds 408 and 425 (Story 4.6).
    - rtds stops treating a 503 or a 429 at reconnect as fatal (Story 4.8).
    - **Where each gets the rule.** rtds calls `polyoxide_ws::retrying_can_fix` with venue's status rule passed in. perps gets it from the `Supervisor`'s default recovery, which reads the class.
  - **R2.** Every socket connect has a timeout, 10 s by default, through the kit's `connect` or `with_timeout`. rtds and perps have none today.
  - **R3, the close reply.** The kit's code is done: `Connection` flushes the RFC 6455 reply when the server closes, and `close()` sends a close.
    - **The wire check (2026-10-10, perps' host).** With pongs withheld, the host sent `1011 pong_timeout` after 70 s. The old bare pattern then wrote 0 bytes, and one flush wrote the 42-byte reply.
    - **Where it takes effect.** The fix reaches a venue only when Stories 4.6–4.9 move that venue onto `Connection`. The S1 notes name R3 there.
  - **R9.** The live tests call the shared TLS function. Three of them call `install_default` themselves:
    - sports `live_api` (Story 4.8);
    - Binance `live_ws` (Story 4.7);
    - clob `live_ws` (Story 4.9).
  - **In the S1 notes.** R1, R2 and R3 change behaviour users can see, so the notes name them, along with Epic 3's R4.
  - **R6 waits for S2.** It renames rtds' `test-fixtures` feature to `test-server`.
- **DFR rows stay per venue**, at the venue's `Protocol` or call site. Only identical or parametric copies merge.
  - **D4, ping schedule.** rtds pings on a quiet tick; perps and Binance ping on the wall clock; sports sends no pings; clob pings every 10 s. Never copy the rtds pump shape to a host that requires pings.
  - **D5, liveness for staleness.** sports counts pings, rtds counts only decoded events, Binance counts any frame, and perps counts frames plus an ok pong.
  - **D6, backoff reset.** What counts as a delivered connection follows D5. It reaches `Backoff::after_connection_ended(delivered)` as a `bool` from the call site.
  - **D7, supervision shape.** sports is a `Stream` state machine and rtds a `run(handler)` future. Both stay outside the `Supervisor`.
  - **D8, outage markers.** prader folds Binance's `Disconnected{path}`→`Reconnected{path}` pairs. perps has no `Disconnected`. rtds signals a reconnect with a new `Snapshot`.
  - **D9, Binance limits.** Rotate at 23 h 50 min. Replay paced at 200 names per request and one request per 200 ms, with the 1024-stream cap enforced first.
  - **D10, perps.** `Recovery::Retry` and `SequenceRegressed` stay perps-specific.
  - **D11, rtds.** Snapshot and backfill semantics, and the correction of the mislabelled spot snapshot.
  - **D12, clob.** clob decodes Binary frames as text.
  - **D13, two backoffs.** Core's jittered `RetryConfig::backoff` is never unified with the socket `Backoff`.
  - **D18, dependency weight.** rtds and sports build nothing heavy.
  - **D19, use after close.** After `AlreadyClosed`, perps and rtds stop, while Binance and sports reconnect. This stays in each venue's recovery, not in `retrying_can_fix`.
  - **Sports and `Capacity`.** Sports retries a connect that failed with `Capacity`, but `retrying_can_fix` gives up on it. Story 4.8 either keeps sports' rule at its call site or names the change as a DRIFT row.
- **Credential-free fence (AD-2, NFR2).**
  - **`polyoxide-ws`'s dependencies.**
    - Normal: tokio (`net`, `time`), tokio-tungstenite, futures-util (`sink`), thiserror, tracing and rustls. serde and serde_json are allowed as optional dependencies but unused.
    - `test-server` adds only tokio's `macros` and `rt`.
    - It depends on neither `polyoxide-core` nor `polyoxide-venue`.
    - Venue's one link to it is a path-only dev-dependency, for the table test.
  - **The allowlist check exists.**
    - **What it compares.** `.github/scripts/tests/test_dep_allowlists.py` diffs `cargo tree -p <crate> -e normal --target x86_64-unknown-linux-gnu`, for each feature set, against `ci/dep-allowlists/<crate>[@<features>].txt`. It names each crate that joined or left the tree, and refuses an allowlist that admits a deny-listed crate.
    - **Coverage so far.** `polyoxide-ws.txt` and `polyoxide-ws@test-server.txt` are committed.
    - **Story 4.8** adds the files for `polyoxide-rtds` and `polyoxide-sports`.
    - **After a deliberate change.** Regenerate the files with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts`, and say why in the commit.
  - **Socket-only deny-list.** `reqwest`, `alloy*`, `polyoxide-core`, `governor`, `hmac`, `sha2` and `keyring`. For `polyoxide-ws`, also `polyoxide-venue`.
  - **What a socket-only crate may build.** `polyoxide-ws`'s dependencies, `polyoxide-venue`'s, and its own. Venue's default build depends on nothing (amendment A3-4), so the allowlists follow that, not the longer list AD-3 gives.
- **Test custody (AD-12, NFR7).**
  - **Protected supervision suites:**
    - perps inline: 21 (17 supervision, 3 backoff, 1 defaults);
    - Binance: `tests/supervision.rs` (21), `supervision_edges.rs` (7), the 3 tests inline in `usdm/ws/supervised.rs`, and the usdm ws client's inline tests;
    - rtds: `tests/supervision.rs` (4) plus 15 inline, 19 in all;
    - sports: `supervision.rs` (15), `bare.rs` (6) and 6 inline, 27 in all.
  - **Also protected:** Story 2.2's class tables, the `tests/failure_tags.rs` twins (Binance, sports, rtds), the ws wire agreement and the live drift detectors.
  - **How tests move.** A moved test keeps its name and assertions. Each moving PR reports per-suite counts before and after.
  - **Backoff tests.**
    - rtds' seven `backoff_schedule` tests are among its 15 inline tests, and the kit already carries them by name as the canonical `Backoff` suite. When Story 4.8 removes rtds' copy, its count report shows those tests moved, not lost.
    - perps', Binance's and sports' own backoff tests have other names, and each must survive by name when its venue's copy goes.
  - **Old loops.** An old supervision loop is deleted only once its suites pass against the `Supervisor`.
- **Removals (amended by A4-1).**
  - **No shims or re-exports.** Every removed public item goes in `docs/s1-removals.md`, which stays cumulative against `v0.38.1`. Bundle K removed nothing.
  - **Each venue's `test_server` module.** Stories 4.6–4.9 each remove their venue's module (perps', Binance's, rtds' and sports') and list it there. clob has two ad-hoc loopback servers instead, which Story 4.9 replaces.
  - **Doc-hidden paths.** Those modules are on the file's "Doc-hidden paths consumers import" list, which CI compiles. Removing one means marking its entry `**Removed**`, with the story and its replacement, `polyoxide_ws::test_server`.
  - **Also removed:** perps' and Binance's `supervised` modules, `MembershipHandle` and `Recovery`.
  - **What keeps its name.** Fixtures stay with their venues. rtds' `fixtures` and its `test-fixtures` feature, which today also gates its `test_server`, keep their names until S2.
  - **Error enums.** Their names do not change until S2. Changed variants are listed for prader.
- **Standing rules (AD-21).** CLAUDE.md is edited in the change that supersedes each rule. Text inside generated regions changes only through `[package.metadata.polyoxide]` and `python3 scripts/gen_registry.py --write`.
  - **What CLAUDE.md already says, after bundle K.** It describes the kit as "no venue uses it yet" and lists what each of Stories 4.6–4.9 adopts. It also describes the shared server and the allowlist check. Each later story rewrites the sentences it makes false.
  - **Story 4.9** removes the last per-crate `ensure_crypto_provider`, so it rewrites the copies text ("each with its own `ensure_crypto_provider` copy", rtds' "own copy", "WebSocket TLS needs a nudge").
  - **Workspace dependencies of rtds and sports.** "They depend on nothing else in the workspace" becomes "they depend on `polyoxide-ws` and `polyoxide-venue`". That line comes from each crate's metadata `notes`.
  - **Socket classification.** "The per-crate match over `tungstenite::Error` stays until Story 4.10" ends with Story 4.10.
  - **perps' idle rule (Story 4.6).** CLAUDE.md and `docs/specs/perps/OBSERVED.md` say the host idle-closes after 60 s without an inbound message. The R3 probe saw something different:
    - the host sends a protocol Ping every 10 s;
    - a client that reads (and so pongs) was never closed in 180 s;
    - with pongs withheld, the host closed with `1011 pong_timeout` after about 70 s.

    Story 4.6 records this in both files. Whether perps counts `post/ping` as liveness was not measured.
- **Gates (NFR9).** A red job withholds the release:
  - clippy and rustdoc with `-D warnings` (a `pub` doc must not link a `pub(crate)` item);
  - MSRV 1.91;
  - `cargo hack` on each feature;
  - the publish dry run;
  - the removals job;
  - the scripts job (allowlists, dependency fences, classify coverage, `gen_registry.py --check`).

## Technical Decisions

- **What `polyoxide-ws` provides (bundle K):**
  - **TLS (W1).** `ensure_crypto_provider`.
  - **Backoff (W3).** `Backoff` and `MIN_BACKOFF`: clamp, doubling, ceiling and reset. The reset input is the call site's `delivered` bool (D6).
  - **Timing (W4).** `Timing` is `#[non_exhaustive]` with public fields: ping (optional), staleness, initial and maximum backoff, connect timeout, and an optional maximum connection age.
    - `Timing::default()` is a 20 s ping, 30 s staleness, 500 ms–60 s backoff, a 10 s connect timeout and no rotation. That is perps' set plus R2's timeout.
    - The other venues override it: rtds pings on a 5 s quiet tick, sports has no ping and a 45 s staleness, and Binance rotates at 23 h 50 min.
    - `timing_setters!` gives a builder any of `ping_interval`, `stale_after`, `backoff`, `connect_timeout` and `max_connection_age`, each writing `self.timing`.
  - **Deadlines (W5).** `next_wait` takes the minimum of the deadlines, saturating, with `Duration::MAX` meaning never.
  - **The error (W6, A4-2).** `WsError` is an opaque boxed struct with the tungstenite error as its `source()`.
    - It is built by `From<tungstenite::Error>` and by the kit's constructors for its own failures: a connect timeout, a server close and an unanswered request.
    - It is read through `kind() -> Kind<'_>`, a `#[non_exhaustive]` view whose variants are the table's rows below. `Refused` carries the raw `Retry-After` header.
    - No crate outside ws matches on tungstenite's error.
  - **Connect (W8).** `connect(request, timeout)` uses `DEFAULT_CONNECT_TIMEOUT` (10 s) and returns the `Socket` alias. `with_timeout(timeout, fut)` bounds a venue's own connect future, as clob's per-address connect needs.
  - **Whether to reconnect (W7, A4-3).** `retrying_can_fix(err, retriable_status)` works as follows:
    - a `Refused{status}` gets the injected rule's answer;
    - `Url`, `HttpFormat`, `Tls`, `AttackAttempt` and `Capacity` are `false`;
    - everything else is `true`.

    Callers pass venue's rule, `|s| class_for_handshake_status(s).is_retriable()`, so the status set has one definition.
  - **The bare tier (W9, W11).** `Connection` is one connection as a `Stream` of frames.
    - `Mode::Data` yields Text and Binary only; pings are still answered. `Mode::All` yields every frame, with the close last.
    - It drains a buffered queue first.
    - `closed()` gives the server's close as `Kind::Closed`.
    - It flushes the close reply (R3), and `close()` sends a close.
    - `last_inbound()` counts every frame, those read while awaiting an answer included (Binance's liveness rule, D5).
  - **Request correlation (W12)** is on `Connection`. The envelope is the caller's: a builder writes the request for its id, and a matcher sorts each frame into answer, stale answer or push. Pushes are kept and yielded in order.
    - **Optional limits.** Pacing and an answer timeout (`NoAnswer`) are both optional. perps sets neither today; Binance uses 200 ms and 10 s.
    - **After a `NoAnswer`,** replace the connection rather than reuse it.
  - **Re-export.** `pub use tokio_tungstenite::tungstenite`, so venues name `Message`, `CloseFrame` and `http` through ws.
  - **Logging.** Everything logs under the `polyoxide_ws` target. Libraries never install a subscriber.
- **Socket error table (AD-15 as amended by A4-2).** `impl_ws_classification!()` takes no arguments.
  - **What it expands to.** In the invoking module, private `fn ws_class(&WsError) -> Class` and `fn ws_is_fault(&WsError) -> bool`, using only `$crate::` paths for venue items.
    - A venue's `Classify` impl calls them in its `WsError` arm.
    - An unused one fails `-D warnings`, so a venue cannot skip the 451 non-fault.
  - **Why a macro.** venue and ws do not depend on each other (AD-2), and in a venue crate both the trait and the type are foreign (the orphan rule).
  - **`Network`:** `Io` (a TLS EOF included), `Protocol`, `ConnectionClosed`, `Other`, `ConnectTimeout`, `NoAnswer`, and any kind added later.
  - **`Closed{code}`** goes through `class_for_close_code`:
    - 1000, 1001, 1006, 1011–1013 and no code are `Network`;
    - 1002, 1003, 1007–1010 and 4000–4999 are `VenueRefusal{code}`.
  - **`InvalidRequest`:** `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity` and `AlreadyClosed`.
  - **`Refused{status, retry_after}`** goes through `class_for_handshake_status(status).with_retry_after(parse_retry_after(v, Duration::MAX))`:
    - a 429 carries its wait, and a zero wait is none;
    - a 418 and a 451 are `Restricted`;
    - a status outside 400–599 is `Decode`.
  - **Faults.** Only a 451 refusal is a non-fault.
  - **The table test.** `polyoxide-venue/tests/ws_classification.rs` pins every row. It also checks that `retrying_can_fix(e, venue_rule) == ws_class(e).is_retriable()` for every refused status from 100 to 599.
  - **Still to do when the venues adopt it (Stories 4.9 and 4.10):**
    - clob's "no addresses resolved" DNS miss, built as `InvalidMessage` today, must reach the table as a `Network` kind;
    - rtds' `Server{status: 451}` becomes a non-fault through the table.
  - **Not carried.** Only a 429 keeps its `Retry-After`. `Class::Restricted` has no wait, so a 418's is dropped, as is every other refused status's. Story 4.10 decides whether to surface them.
  - **Classify coverage.** `WsError` is exempt in `test_classify_coverage.py`, because it is classified only through the venue enums that wrap it. Story 4.10 should add a fence that any crate wrapping `WsError` expands the macro.
- **Classification is separate from recovery.**
  - The class (venue's table) is what consumers and nightly runs read. Whether to reconnect or stop belongs to the `Protocol`'s recovery hook, which by default reconnects only when the class is retriable.
  - For a failed connect, rtds and sports call `retrying_can_fix`. D19 and what a close code means stay with each venue's recovery.
  - Venue-specific recovery, such as perps' `Retry`, moves into its `Protocol`.
  - A `Restricted` failure tags `environmental` when it is not a fault and `real` when it is (A2-1).
- **The `Supervisor` (AD-11, NFR3, Story 4.5).**
  - **What exists.** No `Supervisor`, `Protocol` or recovery type exists yet. Story 4.5 adds them to `polyoxide-ws`, where `Timing`, `Backoff`, `next_wait`, `connect` and `Connection` are its parts.
  - **What a venue implements.** Only a `Protocol`:
    - **Handshake.** A handshake request built on every attempt, so its auth is recomputed each time. Kalshi signs a millisecond timestamp. Auth may go in the handshake or in the subscribe frame.
    - **Frames.** Decode, liveness and delivered predicates that see every frame type: Text, Binary, Ping, Pong and Close.
    - **Pings.** A ping schedule.
    - **Membership.** `P::Membership`, with paced `replay`, and `wanted`.
    - **Optional and venue-specific.** An optional `max_connection_age`, recovery and protocol-originated events, and whether it declares `Disconnected`.
    - **Queue.** Its queue, `Bounded(n)` or `Unbounded`.
  - **Generics.** `Protocol` is generic-only, so ws needs no dynosaur.
- **The `Supervisor`'s invariants:**
  - **Outage markers.** Each outage yields at most one `Disconnected`, and only for a declaring `Protocol`. It yields exactly one `Reconnected`, even when membership empties mid-outage.
  - **Pings and silence.** Pings stay on the wall clock. While the queue is full, the `Supervisor` races `reserve()` against the ping timer and stops reading. Blocked time is not silence.
  - **No drops.** A decoded event is never dropped; a bounded queue backpressures.
  - **`wanted` turning false.**
    - On a healthy connection, it closes politely with no marker.
    - Mid-outage, a declaring `Protocol` first yields the pairing `Reconnected`.
  - **Routed paths.** A router ends a path only by emptying its membership.
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
  - **rtds** keeps `run(handler)` and uses kit blocks only: `Backoff`, connect with a timeout, `retrying_can_fix`, TLS, the close reply and the test server.
  - **sports** keeps its `Stream` state machine, pings as liveness, and its unfixable-refusal rule. It uses kit blocks only.
  - **clob:**
    - it adopts the kit's TLS function, `WsError`, `Connection` and test server, and the shared server replaces its two ad-hoc loopback servers;
    - it keeps Binary-as-text and its 10 s `PING`;
    - `WebSocketWithPing` keeps its behaviour and public shape;
    - its per-address connect stays clob's own, wrapped by `with_timeout`, and is recorded as a new DFR row through `bmad-spec`;
    - clob is not supervised in S1. Its supervised socket is a new type in Epic 7.
- **The scripted server (W13, W15), as built.**
  - **Where it lives.** `polyoxide_ws::test_server`, behind `test-server` and doc-hidden. It is built only from ws' own dependencies, never from `polyoxide-test-support` (AD-1).
  - **How it starts.** `ScriptedServer::start(Vec<Script>)` plays one script per connection, and the last one repeats.
    - `start_answering(scripts, answer)` calls `answer` once per connection. It returns that connection's `FnMut(&str) -> Vec<Message>`, which owns the protocol state.
    - Venue knobs become closure state, not `Script` fields. For Binance these are a held answer, ignoring later requests and refusals. For perps they are refusals, answering pings, the pong status and answering subscribes after the first.
  - **What it covers.** Its actions are the union of the four venue servers' scripts, plus a cut with no close frame. Hold is the default, sports' `silent()`, and `after_first_frame` is rtds' record-before-sending.
    - Its recorders include close replies, pongs and pings received. `wait_for` polls for 5 s.
    - The helpers are `fast()`, `next_item`, `next_event` and `drain`. `fast()` is a `Timing` of 50 ms pings, 300 ms staleness and 10–20 ms backoff.
  - **Self-tests.** The venue servers' self-tests (perps 5, Binance 2, rtds 8) are ported by name. perps' and Binance's answers are written as the closures their suites will pass.
  - **Differences a suite may notice when it switches:**
    - `ping_every` follows sports' shape: the first ping comes one period in, with a big-endian sequence payload. Binance's pinged at once with an empty payload.
    - A scripted close waits up to a second for the client's reply and counts it. Binance's did not wait.
  - **Known gap.** No test reaches `close()` with the reply still queued (deferred).
- **Socket error reshape (Story 4.10).**
  - `PerpsWsError`, `UsdmWsError`, `SportsError`, `RtdsError` and clob's `WebSocketError` each wrap `WsError` and call the macro's functions from their `Classify` impls.
  - Story 2.2's class tables give every case the same class. The only exceptions are R1's deliberate changes to rtds' and perps' handshake statuses, each named in its commit.
  - Binance's `Stopped` should keep its stopping cause. A stream stopped by a 451 refusal then tags `environmental` rather than `real` (deferred from Epic 2).
- **Transport ownership (AD-18).**
  - No venue crate depends directly on tokio-tungstenite or rustls.
  - Venues name its types through `polyoxide_ws::tungstenite`.
  - Story 4.10 extends `.github/scripts/tests/test_dependency_fences.py`, which today covers reqwest and governor, rather than adding a standalone check.
- **The S1 release (AD-16, AD-25, Story 4.11).**
  - **Version.** `release.yml`'s semver job compares against `v0.38.2` and fails a patch bump on a major-level change, so the S1 bump raises the 0.x minor.
    - The bump is a separate commit on `main`, made after `git fetch` and a crates.io check, and it is the last commit before the tag.
    - No bump is ever made on a loom branch.
    - The removals baseline stays `v0.38.1` until S1 ships.
  - **New crate names.** Three are published for the first time: `polyoxide-venue`, `polyoxide-ws` and `polyoxide-cli`. The limit is five per release.
  - **Before the release.**
    - Confirm that the crates.io token covers `polyoxide*`, including publish-new for `polyoxide-cli` (404 today), and that it expires after S3.
    - The `cargo` and `pypi` environments' restriction to `main` is still unset. It is a repository setting the user changes, not the agent.
  - **Removal list.** `docs/s1-removals.md` is final and gives call-site changes beside each path.
  - **Release notes.** A consumer-impact section names DRIFT R1, R2, R3 and R4 and the log targets moved to `polyoxide_ws`.
    - `deferred-work.md` holds about 21 entries addressed to Story 4.11, and they are the notes' source.
    - They cover Epics 1–3's consumer-visible changes and bundle K's additions: the new crate and its public items, and the new macro.
  - **After the release.** A smoke check proves that `cargo install polyoxide-cli` works from crates.io.
  - **Stage line.** The guide's stage line is generated from `[workspace.metadata.polyoxide] stage` (currently `S1`) in the root `Cargo.toml`. Change it there and run `gen_registry.py --write`, as well as recording the stage in the spine.

## Cross-Story Dependencies

- **Prerequisites, all landed:**
  - Story 2.6 (socket live suites on the tag reporter);
  - Story 1.7 (the removal gate);
  - Stories 2.1 and 2.2 (`Classify`, the status and close-code maps, the socket `Classify` impls);
  - Story 3.13 (the dependency fence);
  - Stories 4.1–4.4 (bundle K: the kit, the table and the server), on `aidanb/restructure-run`, not yet on `main`.
- **Within Epic 4:**
  - 4.5's unit suite runs against the shared server, using a test `Protocol`.
  - 4.6 and 4.7 need 4.5. 4.8 and 4.9 need only the kit.
  - **Per A4-1, each of 4.6–4.9 makes three changes in order:**
    - it switches its venue's suites onto the shared server under the old loop, proving the server on its own;
    - then it moves the loop, or adopts the kit's blocks;
    - then it removes the venue's `test_server` and lists it.
  - **Bundles.** L1 is 4.5 and 4.6, which prove the `Supervisor` on perps first. L2 is 4.7 and 4.8. Then come 4.9, 4.10 and 4.11.
  - **4.10** needs 4.3, plus the migrations (4.6–4.9): perps' `Retry` must already live in its `Protocol`, and no venue may still use tungstenite directly.
  - **4.11** needs Epics 1–4 complete and CI green on `main`.
- **Later epics:**
  - Epic 5 (S2, one loom integration session) moves the socket crates into `polyoxide-polymarket`. It also does R6, renames the error enums, and runs AD-2's check per feature.
  - Epic 7's supervised clob market and user sockets build on 4.5 and 4.9.
  - Epic 8's Kalshi socket uses the `Supervisor`, with per-attempt handshake auth.
- **Spine.**
  - Amendments A2-1, A3-1 to A3-5 and A4-1 to A4-3 are accepted but not merged into the spine. They override the spine and the epics file where those differ. Three places differ:
    - Story 4.4's second criterion, which A4-1 moves to 4.6–4.9;
    - AD-15's rows named for tungstenite's variants, replaced by `Kind` (A4-2);
    - W7's and AD-11's injected rule for closes as well as refusals, which A4-3 narrows to refused statuses.
  - No story is assigned to the merge session.
