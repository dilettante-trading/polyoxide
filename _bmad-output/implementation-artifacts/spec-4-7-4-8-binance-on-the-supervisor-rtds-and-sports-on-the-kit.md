---
title: 'Stories 4.7 and 4.8: Binance on the Supervisor, rtds and sports on the kit'
type: 'feature'
created: '2026-10-11'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '96f96ec94d630293c11026b558d73bae8b680aa5'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-4-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-4-5-4-6-supervisor-and-perps-on-it.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Binance (`polyoxide-binance/src/usdm/ws/supervised.rs`, 1,018 lines) still runs its own supervision loop. Its bare tier, rtds and sports each carry their own copies of the kit's blocks (TLS, `Backoff`, connect, test server). rtds also lacks R1 (a 503 or 429 at reconnect ends `run()`), R2 (no connect timeout) and R3 (no close reply). Three live suites install TLS themselves (R9).

**Approach:** In A4-1's order:
- **Binance (4.7).** Switch its suites onto `polyoxide_ws::test_server` under the old loop. Then move the bare tier onto the kit's `connect` and `Connection`, and the supervised tier onto `Supervisor<UsdmProtocol>` behind a Binance router. Then delete the old loop, server and copies.
- **rtds and sports (4.8).** Each switches its suites first, then adopts the blocks without the Supervisor (D7), then deletes its copies.

**Decisions (made as the user's delegate, 2026-10-11):**
- **L2-D1, B3 settled with no kit change.** The 10 s answer timeout is the bare tier's only (`client.rs:28`). The supervised tier already uses `stale_after` as its answer timeout (`supervised.rs:273-278`).
  - On the Supervisor, the kit's `stale_after` bound on `apply` and `replay` is W12's path: a stalled change replaces the connection. Binance sets no answer timeout on the supervised `Connection`.
  - **Keeping the old answer.** The old loop answers `Ok(())` to a change whose connection died under it, records it, and replays it (`supervised.rs:916-951`), so the protected edge tests still pass:
    - `apply` records a change before it sends it, as the old loop recorded it;
    - an `apply` whose request fails because the connection did (a close or a failed read) records the change and returns `Ok(())`;
    - Binance's `MembershipHandle` turns the `stalled` answer into `Ok(())`. That answer is built by `UsdmProtocol::stalled`, and the handle maps no other error.
  - The outage's reason may now read `Stale` or `Closed` rather than `Error(NoAnswer)`.
- **L2-D2, the full-queue risk is accepted, with no kit change.**
  - **Why not answer pings while full.** tungstenite yields frames in order, so answering the host's Pings means reading past data frames into an unbounded buffer, which defeats the bound.
  - **Never worse than the old loops.** Binance's and perps' old loops stopped reading and pinging, and replaced the connection as stale after `stale_after` (Binance 30 s).
  - **What changes.** Under the Supervisor a slow consumer is never stale: the client's pings continue. Binance's host closes after 10 min with no pong (documented, not measured), and perps' after about 70 s. The close is reconnected, and queued events are kept.
  - **Where it is recorded.** Binance's builder docs say so, and the deferred RISK entry is closed with this reasoning.
- **L2-D3, two small kit changes.** `polyoxide-ws` is unpublished, so neither needs a removal entry.
  - **`Protocol::configure(&self, Connection) -> Connection`.** Defaults to the identity. `attempt` calls it right after `Connection::new`, so a venue can set `min_request_interval`. Binance sets 200 ms.
  - **`Outage<E>` owns its error.** `Outage::Error(E)` and `Closed { code, reason: String }` replace the borrowed form, so Binance's `DisconnectReason::Error(UsdmWsError)` keeps its payload.
- **L2-D4, Binance's consumer API.**
  - **Kept, at `usdm::ws`:** `UsdmWsBuilder`, `SupervisedUsdmWs`, `MembershipHandle` (now the router's handle over per-path kit handles), `Event`, `DisconnectReason` and `UsdmWs`.
    - The router keeps the module name `usdm::ws::supervised`, so no path is removed for it.
    - `UsdmProtocol` is `pub(crate)`.
    - The builder holds a `polyoxide_ws::Timing` with setters from `timing_setters!`, as perps' builder does. Its tests read `timing.<field>`.
  - **Changed:**
    - `UsdmWsError::Connect` wraps `polyoxide_ws::WsError`.
    - `Classify` expands `impl_ws_classification!`, with Binance's handshake `403 → Restricted` (D14) arm ahead of it.
    - `From<tungstenite::Error>` is gone. The `From<WsError>` mapping sends `Kind::Closed` to `Closed`, `ConnectTimeout` to `ConnectTimeout`, and the rest to `Connect`. The bare tier maps a request's `NoAnswer` to `NoAnswer { id, timeout }` itself.
    - `stopped()` is `UsdmWsError::Stopped`.
  - **Removed, each listed in `docs/s1-removals.md`:** `Recovery` (both paths), `UsdmWsError::recovery()`, and the doc-hidden `test_server` (**Removed**). `fixtures` and the `test-server` feature stay.
- **L2-D5, Binance's rows in `UsdmProtocol`.**
  - **Routing:** one `Supervisor` per routed path.
  - **Pings and liveness:** D4 sends an empty `Ping` on the wall clock. D5 counts every frame as live.
  - **Delivered (D6):** an `Update` was decoded.
  - **Outage markers (D8):** `Disconnected { path, reason }` and `Reconnected { path }`.
  - **Rotation and pacing (D9):** rotate at 23 h 50 min via `max_connection_age`. A replay sends at most 200 names per request, paced 200 ms, with the 1024-stream cap checked first.
  - **During an outage:** `record` returns `Recorded(Ok(()))`, and `start` opens a new path's first connection.
  - **`recover`:** `AlreadyClosed` reconnects (D19), `Capacity` reconnects, and a refused handshake follows `retrying_can_fix` with venue's rule. `Frame` is `Skip`. `Refused`, `TooManyStreams`, `WrongPath` and `Stopped` are `Stop`.
  - **The consumer's queue:** one bound of 1024 shared by both paths, as today.
  - **Not wanted:** an unsubscribe of a path's every stream sends nothing and closes the path politely.
- **L2-D6, rtds wraps `WsError` now** (its part of 4.10, as perps' was), because R1 and R2 need it.
  - **The error:** `RtdsError::Connection(WsError)`, with `Classify` through the macro. `From<tungstenite::Error>` is gone.
  - **Recovery:** rtds keeps `Recovery` and `recovery()`, since it is not on the Supervisor. Its `Connection` arm is `AlreadyClosed → Fatal` (D19), then `Capacity → Reconnect`, then `retrying_can_fix` with venue's rule.
  - **Reading:** rtds reads through `Connection` (`Mode::Data`), which brings R3, and keeps its quiet-tick `"PING"` (D4) and its decoded-only liveness (D5).
  - **The builder:** `RtdsBuilder::connect_timeout` (default 10 s) is new. rtds does not adopt `Timing`, so its builder tests stay unchanged.
- **L2-D7, sports keeps `SportsError` until 4.10.** Its R1–R3 hold already, and its protected tests pin `Box<tungstenite::Error>`.
  - **Adopted from the kit:** `ensure_crypto_provider`, `Backoff`, `with_timeout` with `DEFAULT_CONNECT_TIMEOUT`, the `Socket` alias and the shared server.
  - **Its own:** sports keeps its `retrying_can_fix` (which retries `Capacity`) and its raw read loop.
  - **For 4.10:** a deferred entry gives sports the kit's `connect`, `Connection` and `retrying_can_fix`.
- **L2-D8, the behaviour changes.** Each is named in its commit and in 4.11's release notes:
  - **Binance:**
    - R3 on the bare tier;
    - a full queue is no longer silence (L2-D2);
    - the outage reasons of L2-D1;
    - supervision logs move to `polyoxide_ws`;
    - a rotation resets the backoff after a connection that delivered;
    - the replay is bounded as a whole by `stale_after`;
    - a refused 429's `Retry-After` is surfaced by the macro;
    - error text.
  - **rtds:**
    - R1, R2 and R3;
    - a 451 refusal is not a fault;
    - the bare `Rtds` ends after its first transport error;
    - the close log moves to `polyoxide_ws`;
    - error text.
  - **sports:** none but R9.
  - **All three:** R9.
- **L2-D9, the custody exception.** The failure-tag twins whose error now wraps `WsError` change only their text and Debug assertions: Binance's `handshake_503`, `handshake_404` and `handshake_451`, and rtds' `tls_eof`, with their Python rows. Their tags are unchanged.

## Boundaries & Constraints

**Always:**
- **Test custody (AD-12).** A moved test keeps its name and assertions. Its setup and calls may change. L2-D9 is the only assertion exception.
  - **Counts at 96f96ec, reported before and after per suite:**
    - Binance: `tests/supervision.rs` 21, `supervision_edges.rs` 7, and inline `supervised.rs` 3, `client.rs` 9, `error.rs` 6, `stream.rs` 6, `event.rs` 6, `test_server.rs` 2; `failure_tags.rs` 17; CLI `tests/ws_binance.rs` 13.
    - rtds: `tests/supervision.rs` 4, and inline `supervisor.rs` 15, `client.rs` 12, `error.rs` 12, `test_server.rs` 8; `failure_tags.rs` 1.
    - sports: `supervision.rs` 15, `bare.rs` 6, and inline `supervised.rs` 6, `client.rs` 7, `error.rs` 9; `failure_tags.rs` 7.
  - **Already in the kit by name.** Some tests duplicate the kit's under the same name: rtds' 7 `backoff_schedule` tests, rtds' 8 server tests, Binance's 2 server tests, and rtds' TLS test. Each is deleted with its copy, and the report shows it moved.
  - **Kept by name.** Binance's 2 and sports' 4 differently named backoff tests stay, now on `polyoxide_ws::Backoff`.
  - **Moved recovery tests.** Binance's `recovery()` tests move to tests of `UsdmProtocol::recover`, as perps' did.
- **The order.** The server switch comes first, then the move, then the deletion, in separate commits (A4-1). A DRIFT row gets its own commit and test.
- **CLAUDE.md (AD-21)** changes in the commit that changes the rule. Generated text comes from `python3 scripts/gen_registry.py --write` after metadata changes, in the same commit.
- **Gates.**
  - `scripts/api_removals.py check` is green.
  - The rtds and sports allowlists pass, with the deny-list crates absent.
  - rtds and sports still build no `polyoxide-core`, `reqwest` or `alloy`.

**Never:**
- No change to clob, perps or core: perps' code changes only as far as L2-D3's kit changes force it.
- No shims or re-exports of a removed path.
- The socket `Backoff` and core's backoff are not merged (D13).
- No release, no version bump, no push.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected |
|---|---|---|
| Binance change, connection dies | `ignore_later_requests`; subscribe, then unsubscribe | the caller gets `Ok(())`; `Disconnected` then `Reconnected`; the replay carries the change |
| Binance change refused | `refuse` on the subscribe | `Err(Refused)`; the connection is kept and keeps delivering |
| Binance path emptied, healthy | unsubscribe the path's last stream | no UNSUBSCRIBE is sent; a polite close; no marker; the other path is unaffected |
| Binance path emptied, mid-outage | unsubscribe during the backoff | `Reconnected { path }` once; the merged stream continues |
| Binance stop on one path | a refused replay | the error, then the whole stream ends; the handle answers `Stopped` |
| Binance slow consumer | 1024 unread items | client pings continue; no `Stale` reconnect; no item is dropped |
| rtds 503 / 429 on reconnect | refuse-upgrade on the second attempt | retried (R1); `run()` continues |
| rtds hung handshake | stall | `ConnectTimeout` after `connect_timeout` (R2); retried at reconnect |
| rtds server close | close with a code | the close reply is sent (R3); reconnect |
| rtds `AlreadyClosed`, `Capacity` | either error | `Fatal`, `Reconnect` (unchanged) |
| sports `Capacity` at connect | the error | retried, as today |

</frozen-after-approval>

## Code Map

- **The kit (`polyoxide-ws`)**
  - `src/supervisor/mod.rs`:
    - `Protocol` `:130`; `Outage` `:84` (L2-D3); `stalled` and `stopped` `:239-248`;
    - the basis note `:282-285`, which names Binance's file and must say it was deleted in 4.7.
  - `src/supervisor/task.rs`:
    - `attempt` `:183`, with `configure` going after `:190`;
    - `declare` uses `:288`, `:336`, `:344`, `:359` and `:372`;
    - `Task::apply` `:543-572`, `deliver` `:578-616`.
  - **Available:**
    - `connect`, `with_timeout` and `retrying_can_fix` (`connect.rs`);
    - `Connection` (`bare.rs`) with `min_request_interval` and `answer_timeout` (`request.rs:73-83`, by value);
    - `request` and `await_answer`;
    - `test_server` (`start_answering` for answer state, `after_first_frame`, `close_with`, `then_close`, `refuse_upgrade`, `stall`, `hold`). Its `Received.text` is a `String`, so the adapters parse it.
  - **Basis notes naming files that 4.7 and 4.8 delete:** `tls.rs:17-19`, `backoff.rs:18-19`, `connect.rs:29-30,71-73`, `bare.rs:52`, `error.rs:20-25`, `timing.rs:17-19`, `test_server/mod.rs:5-8` and `helpers.rs:3-5`.
- **Binance (`polyoxide-binance/src/usdm/ws/`)**
  - `supervised.rs`:
    - router `supervise` `:466-511`; subscribe and unsubscribe routing `:538-666`;
    - start in an outage `:714-741`; record during the backoff `:827-857`; changes on a live path `:916-951`;
    - rotation `:889`; polite close `:691`; `events.closed()` `:492`, `:912`;
    - `Backoff` `:46-77`; `EVENT_BUFFER` 1024 `:41`;
    - the rustdoc example using `Recovery` `:369-397`; the `stale_after` and handle docs `:187-191`, `:319-321`.
  - `client.rs`:
    - `ANSWER_TIMEOUT` `:28`; its own connect `:153-174` and `ensure_crypto_provider` (`mod.rs:85-97`);
    - `request`, `await_answer` and `await_pong` `:291-378`, with pacing `:302-304`; `poll_next` `:381-415`, which never sends the close reply.
    - For the `NoAnswer { id }` text, take the id from `next_id()` before the request.
  - `error.rs`: `recovery` `:118-149`; `transport_class` `:159-174`; `Classify` `:185-210`.
  - `test_server.rs`, and what its knobs become on the shared server:
    - `pushes` is `send` plus `.after_first_frame()`;
    - `refuse`, `ignore_later_requests` and `hold_answer` become answer-closure state; models are in `polyoxide-ws/src/test_server/tests.rs:389-441` and `request.rs:484-504`;
    - `ping_every` starts one period in, with a sequence payload, which is harmless here;
    - `close_after` plus `close_code` is `then_close`/`close_with`, and waits 1 s for the reply.
  - **Adapters.** The integration suites need a `tests/common/` adapter, and the `client.rs` inline tests a `cfg(test)` one, or one file shared through `#[path]`. Neither is public.
- **Binance's users:**
  - `polyoxide-cli/src/commands/ws/binance.rs:232` uses `Recovery::SkipFrame`; test for `UsdmWsError::Frame` instead.
  - `tests/live_ws.rs:137` is R9, and it uses `connect_async` and `UsdmWsError::from(tungstenite)` at `:151-165`.
  - `README.md:69-101` and the `polyoxide-binance` `Cargo.toml` features `ws` and `test-server` (`:13-19`). `polyoxide-ws` becomes an optional dependency under `ws`, and the path dev-dependency gets `test-server`.
  - Drop the direct `tokio-tungstenite` and `rustls` when nothing names them.
  - Binance needs no allowlist (it is not socket-only).
- **rtds (`polyoxide-rtds/src/`)**
  - `client.rs`:
    - TLS `:32-37`; connect `:93-110`;
    - `poll_next` `:152-181`, where the close `:169-170` has no reply; `close` `:141-144`.
  - `supervisor.rs`: `Backoff` `:37-83`; the reconnect loop `:247-267`; the builder `:86-150`; the delay debug `:254`, which `test_log.rs` reads, so keep it.
  - `error.rs`: the variant `:15-17`, `From` `:96-100`, `recovery` `:136-170`, `transport_class` `:176-190`.
  - **Scripts, in order:**
    - `SendThenIdle` becomes `frames(..).after_first_frame()`, which is required on every rtds script;
    - `SendThenClose` becomes `.then_close()`;
    - `RejectHandshake` becomes `reject()`;
    - `SendRawThenIdle` becomes `messages(..)`.
  - **Recorders:**
    - `received_subscriptions` becomes `first_frames`;
    - `wait_for` waits 5 s.
  - `Cargo.toml`: drop `tokio-tungstenite` and `rustls`; add `polyoxide-ws` as a normal dependency and as a path dev-dependency with `test-server`. Keep `test-fixtures`.
  - Reword the metadata note at `:58-62`.
  - `lib.rs:34-44` holds the `test_server` module and its comment.
- **sports (`polyoxide-sports/src/`)**
  - `client.rs`: TLS `:37-42`; `open` `:47-58`, which becomes `with_timeout(t, connect_async(..))`; `Socket` and `DEFAULT_CONNECT_TIMEOUT` `:23-26`.
  - `supervised.rs`: `Backoff` `:51-90`, whose 4 tests at `:375-413` keep their names.
  - **Scripts:** `Script::silent()` becomes `hold()` at 9 call sites (`supervision.rs:90,111,224,246,366`; `bare.rs:24,40,55,104`). A test-local `Silent` trait may keep the bodies.
  - **R9:** `tests/live_api.rs:60` calls `polyoxide_ws::ensure_crypto_provider()`. `scripts/live_unwraps.baseline.json` must not rise.
  - `Cargo.toml`: drop `rustls`, keep `tokio-tungstenite`, keep `test-server = []` for `fixtures` (the CLI imports them), and reword the note at `:54-59`.
- **Shared files**
  - `ci/dep-allowlists/`: create `polyoxide-rtds.txt`, `polyoxide-rtds@test-fixtures.txt`, `polyoxide-sports.txt` and `polyoxide-sports@test-server.txt` empty, then `--write` (it rewrites only files that exist).
  - `docs/s1-removals.md`: `:84-85` (Binance), `:89` (rtds) and `:91` (sports) are marked **Removed**.
  - `.github/scripts/tests/test_classify_failures.py:334-337`: the rtds row, and any row naming Binance's handshake twins.
  - **CLAUDE.md:**
    - the `:180` per-crate tungstenite sentence;
    - `:356-372` (Binance streams);
    - `:461` (the shared server);
    - `:501`;
    - `:518` (the `ensure_crypto_provider` copies);
    - `:520` (the kit paragraph);
    - `:528-536` (the sports and rtds dependencies and the TLS copy);
    - the generated `:79-80` and `:549`.

## Tasks & Acceptance

**Execution.** One commit each, in order. Run fmt, clippy and the touched tests before each.
- [x] **L2-1 (4.7, the server switch).** Binance's `tests/supervision.rs`, `supervision_edges.rs` and `client.rs` inline tests run on `polyoxide_ws::test_server` through adapters, under the old loop. Record the counts before and after.
- [x] **L2-2 (kit, L2-D3).** Add `Protocol::configure` and the owned `Outage<E>`, with a kit test for `configure`. Adjust the kit's test protocol.
- [x] **L2-3 (4.7, the bare tier, DRIFT R3).**
  - `UsdmWs` runs on `connect` plus `Connection` (`Mode::Data`), with the 200 ms pacing, the 10 s answer timeout and the correlation.
  - Make the L2-D4 error changes, with the L2-D9 twins and their Python rows.
  - Add a test: `a_server_close_is_answered_with_the_close_reply_drift_r3`.
- [x] **L2-4 (4.7, the loop).** `usdm/ws/protocol.rs` (new, `UsdmProtocol`) and the router in `supervised.rs` run on `Supervisor`, under L2-D1, L2-D4 and L2-D5.
  - The suites pass unchanged.
  - Each matrix row for Binance, and each L2-D8 change for Binance, has a test.
  - Update the builder docs (L2-D2).
- [x] **L2-5 (4.7, deletion).**
  - Delete the old loop, the `Backoff`, `ensure_crypto_provider`, `test_server.rs` and `Recovery`.
  - Fix the CLI arm and the README.
  - R9 in `live_ws.rs`, and drop the direct `tokio-tungstenite` and `rustls`.
  - Update `s1-removals`, run the gate, and update CLAUDE.md.
- [x] **L2-6 (4.8, rtds' server switch)**, under the old code.
- [x] **L2-7, L2-8 and L2-9 (rtds' DRIFT R2, R1 and R3, one commit each).**
  - Make the L2-D6 changes.
  - Each row gets its named test: `..._drift_r2`, `a_503_or_429_refusal_on_reconnect_is_retried_drift_r1` and `..._drift_r3`.
- [x] **L2-10 (rtds, deletion).**
  - Delete the TLS, `Backoff` and `test_server.rs` copies.
  - Update Cargo, the allowlists and the metadata, then run `gen_registry`.
  - Update `s1-removals` and CLAUDE.md.
- [x] **L2-11 (4.8, sports' server switch)**, then **L2-12 (sports: the blocks, R9 and the deletion)**, as L2-D7 sets out, with the allowlists, metadata, `s1-removals` and CLAUDE.md.
- [x] **L2-13 (docs).**
  - `deferred-work.md`: the 4.11 entries for L2-D4, L2-D6 and L2-D8; close the RISK and B3 entries (L2-D1, L2-D2); add the 4.10 entry for sports (L2-D7).
  - Kit basis notes that name deleted files.

**Acceptance Criteria:**
- Given the suites in Boundaries, when L2-5, L2-10 and L2-12 land, then every moved test passes with its name, and with its assertions unchanged except under L2-D9, and the counts reconcile.
- Given `scripts/api_removals.py check`, when the bundle is done, then it is green and every removal is listed.
- Given `cargo tree -p polyoxide-rtds` and `-p polyoxide-sports`, when the allowlist test runs, then it passes with no denied crate.
- Given the three live suites (Binance `live_ws`, rtds `live_api`, sports `live_api`), when run with `-- --ignored`, then each passes or fails tagged environmental or transient, recorded.
- Given `git diff 96f96ec -- polyoxide-clob polyoxide-core`, when the bundle is done, then it is empty.

## Implementation Notes

Commits, in order: L2-1 fdec80a, L2-2 02636f1, L2-3 8e4ded5, L2-4 9b0e314, L2-5 344a404, L2-6 62d69b4, L2-7 958d8fa, L2-8 4c7969c, L2-9 cd0cb74, L2-10 bb7d670, L2-11 da90545, L2-12 b37f7ba, then L2-13 (docs).

**Binance's protocol and router (L2-4), the details left to the implementer:**
- **`UsdmProtocol`** (`usdm/ws/protocol.rs`, crate-private) takes `type Ack = Vec<StreamName>`, the path's streams after the change, from both `apply` and `record`. The router keeps its view of each path's streams from that answer, and after a refusal reads it back with an empty `Subscribe`, which sends nothing. So its view is exact after every change, where the old router missed the batches answered before a refusal (`a_partly_refused_subscribe_keeps_the_batches_answered`).
- **`apply`** checks the cap, records the change, sends batches of 200 paced by the connection (`configure` sets 200 ms), undoes a refused batch with those after it, keeping the ones answered, and answers `Ok` with the change recorded when the connection fails under a request. An unsubscribe of the path's last stream sends nothing; the kit then closes the connection as no longer wanted. `record` checks the cap too.
- **`stalled`** is `UsdmWsError::NoAnswer { id: 0, timeout }`. The router maps it to `Ok`, only for an existing path's answer (`Supervisor::start` never returns it), and maps nothing else. A replay that outlives `stale_after` gets the same error, which `recover` reconnects.
- **The router** holds `(generation, kit handle)` per path. A forwarder task per path moves its `Supervised` stream (`Queue::Bounded(1)`) into the shared 1024 queue and reports the path's end with `close()`'s result. A path whose streams leave is forgotten at once; its forwarder keeps draining (so a mid-outage `Reconnected` is not lost) and its later end report is ignored by generation. A `watch` channel closes every forwarder when the router ends.
- **`disconnected`** maps `Outage::Stale` and any later kind to `DisconnectReason::Stale`, as `Outage` is `#[non_exhaustive]`.

**Where the code departs from the plan, and why:**
- **`ensure_crypto_provider` went with its last caller**: Binance's in L2-3, rtds' in L2-7, rather than in L2-5 and L2-10, as perps' did in L3. rtds' rustls dependency went with it in L2-7; Binance's stayed until L2-5 for the live suite's raw socket.
- **rtds' `polyoxide-ws` dependency, its registry note and the regenerated regions came in L2-7**, not L2-10: R2's connect needs the kit, and `gen_registry.py --check` fails on any stale region. sports' note ("like `polyoxide-rtds`") became false there too, so it changed in L2-7 and again in L2-12.
- **Binance's old loop sat unbuilt in `usdm/ws/legacy_supervised.rs` for one commit** (L2-4), since the router keeps `supervised.rs`; L2-5 deleted it, as perps' L4 and L5 did.
- **CLAUDE.md**: L2-1 left two sentences stale (the shared server's "stay until" list and which server Binance's offline tests drive); L2-4 fixed them, and its message says so.
- **One adapter file for Binance**, `src/usdm/ws/scripted.rs`, built under `cfg(test)` and included by the integration suites with `#[path]`, so it names nothing of the crate's own and carries `#![allow(dead_code)]` (each suite uses part).
- **rtds and sports have no adapter.** rtds' scripts are written against the shared server directly, and sports' read `Script::hold()` for `Script::silent()`; no `Silent` trait was needed.
- **Binance's new tests are in a new file, `tests/supervision_kit.rs`**, so the protected suites keep their counts.
- **The log-target test registers a second `tracing::Dispatch`.** With one dispatcher registered, tracing-core 0.1.36 computes the interest of a callsite another test's thread hits first from that thread's default and caches `never` for every thread; the test failed in about half of its parallel runs before.
- **`UsdmWs::close_frame` became `#[cfg(test)]`**, and its `last_inbound` and the crate-visible `send_ping` went, as only the old loop read them.
- **rtds' and sports' own tokio features are `time` alone**; the kit enables what its blocks need.
- **`the_bare_stream_ends_after_its_first_transport_error` passes on rtds' old reader too** for a cut connection; it pins the rule rather than proving the change.
- **The CLI's `Recovery::SkipFrame` arm was deleted**: the arm above it already matched `UsdmWsError::Frame`, the one error the stream skips past.
- **MSRV was not checked locally** (rustc 1.95.0, no rustup); CI's msrv job is the check.

**Test custody (AD-12).** Counts at 96f96ec and after, by `cargo test -- --list`:

| Suite | Before | After |
|---|---|---|
| Binance `tests/supervision.rs` | 21 | 21, same names, on the shared server and the Supervisor |
| Binance `tests/supervision_edges.rs` | 7 | 7 |
| Binance `usdm/ws/supervised.rs` | 3 | 3 (`timing.*`, the kit's `Backoff`) |
| Binance `usdm/ws/client.rs` | 9 | 10 (+R3) |
| Binance `usdm/ws/error.rs` | 6 | 5: 3 `recovery()` tests moved to `protocol.rs`, +2 (the `From<WsError>` mapping, the 429 `Retry-After`) |
| Binance `usdm/ws/protocol.rs` | — | 6: the 3 moved, `Fatal` read as `Stop` and `SkipFrame` as `Skip`, +3 (D19 and `Capacity`, `record`, `decode`) |
| Binance `usdm/ws/stream.rs`, `event.rs` | 6, 6 | 6, 6 |
| Binance `usdm/ws/test_server.rs` | 2 | 0: both in the kit by name (`test_server::tests::as_binance_will_use_it`) |
| Binance `tests/failure_tags.rs` | 17 | 17 (L2-D9 text only) |
| Binance `tests/supervision_kit.rs` | — | 10 |
| CLI `tests/ws_binance.rs`, `ws_sports.rs` | 13, 16 | 13, 16 |
| rtds `tests/supervision.rs` | 4 | 7 (+R1, R2, R3) |
| rtds `supervisor.rs` | 15 | 9: the 7 `backoff_schedule` tests are the kit's by name (`backoff::tests::backoff_schedule`), +1 (the connect timeout's default) |
| rtds `client.rs` | 12 | 14: the TLS test is the kit's by name (`tls::tests`), +3 (R2, R3, the end after an error) |
| rtds `error.rs` | 12 | 16 (+R1, R2, the 451 non-fault, D19 and `Capacity`) |
| rtds `test_server.rs` | 8 | 0: all 8 in the kit by name (`test_server::tests::as_rtds_used_it`) |
| rtds `tests/failure_tags.rs` | 1 | 1 (L2-D9 text only) |
| sports `tests/supervision.rs`, `bare.rs` | 15, 6 | 15, 6 |
| sports `supervised.rs`, `client.rs` | 6, 7 | 6, 7 (the four backoff tests on the kit's `Backoff`) |
| sports `error.rs` | 9 | 10 (+`Capacity` at connect) |
| sports `tests/failure_tags.rs` | 7 | 7 |

Every name at 96f96ec is present after, in its crate or, for the 18 named above, in `polyoxide-ws` (checked by name). Each suite passed on its old loop or code after its server switch (L2-1, L2-6, L2-11), and the five behaviour-change tests of `tests/supervision_kit.rs` fail on Binance's old loop while its five matrix tests pass on both (checked by building the old loop against them).

**Gates, at L2-12 with L2-13's doc changes:** `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`, `cargo test --all-features --workspace` (109 test binaries and doctests, all green), `RUSTDOCFLAGS=-D warnings cargo doc --workspace`, `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`, the six `--no-default-features --test headers` runs, `cargo publish --workspace --dry-run --no-verify` and `publish_order.py check-manifests`, `uv run pytest tests/` in `.github/scripts` (927, the allowlists included), `gen_registry.py --check` and `live_unwraps.py` all pass. `scripts/api_removals.py check --baseline v0.38.1` (cargo-semver-checks 0.51.0, rustc 1.95.0): 46 removals, 46 listed, every listed path compiles; `target/semver-checks` was deleted afterwards. `git diff 96f96ec -- polyoxide-clob polyoxide-core` is empty, and so is perps'.

**The live suites, 2026-10-11:**
- Binance `cargo test -p polyoxide-binance --features ws --test live_ws -- --ignored` (20:20 UTC): 4 of 4 passed, the supervised tier and the R9 raw socket included.
- rtds `cargo test -p polyoxide-rtds --test live_api -- --ignored` (20:21 UTC): 3 of 3 passed.
- sports `cargo test -p polyoxide-sports --test live_api -- --ignored` (20:22 UTC): 4 of 5 passed. `live_frames_round_trip_and_carry_no_unmodelled_keys`, the host's drift detector, failed untagged (real): NFL frames now carry `turn` and `turnProviderId`, which `MatchUpdate` does not model. A wire change, not the bundle's (only the suite's TLS install changed); filed in `deferred-work.md`.

## Spec Change Log

## Review Triage Log

Review 1, 2026-10-11: three layers (blind B, edge-case E, verification-gap V), 23 findings. Groups: G1–G12 patched, D1–D2 deferred, the rest rejected. The patches are 6c86ccd; afterwards the Verification commands all passed, the removals gate with 46 removals, 46 listed.

| # | Verdict | Group | Evidence |
|---|---|---|---|
| B1 | medium | G1 patch | Every "died under" test scripts `ignore_later_requests`, so the kit's `stale_after` bound answers them; `UsdmProtocol::apply`'s `Err(err) => break` arm (`protocol.rs:192-195`) is never reached, and `return Err(err)` there would pass. The `_l2_d1` test tests a stall, not a death. |
| B2 | low | rejected | `request` fails with a non-`Refused` error only when the connection closed, its read failed (`finished`), or a send failed, all of which end the connection at the next read; a send error that leaves the socket open is not shown reachable, and the fix adds a guard. |
| B3 | low | rejected | Same as E1. The whole-replay bound is L2-D8 (6), documented on `stale_after` and pinned by `a_replay_is_bounded_as_a_whole_by_stale_after`; it bites only a `stale_after` under the pacing (about 1 s for 1024 streams) against the 30 s default, and a guard adds validation. |
| B4 | low | G5 patch | `UsdmWs` now ends after its first transport error (`Connection::fail`), and later control calls fail `Connect(AlreadyClosed)`; rtds' twin change is documented (L2-D8 (13)) and Binance's `UsdmWs` doc and release note say nothing. |
| B5 | low | rejected | L2-D4 sends the kit's `NoAnswer` to `Connect` and has the bare tier map its own with the id; `UsdmProtocol::stalled` builds `NoAnswer` directly, and no path reaches `From` with a `NoAnswer` today. |
| B6 | low | G6 patch | Each wrapper prefixes the kit's `"WebSocket error: {0}"`, so every Binance and perps transport error reads "WebSocket transport error: WebSocket error: …", in every log and CLI message; making the kit's `Transport` display tungstenite's text alone restores each venue's pre-kit wording. |
| B7 | low | G7 patch | `docs/s1-removals.md` points a caller "deciding for itself" at `Classify`, which differs from the removed `recovery()` on `Closed` (any code reconnected; 1008 is `VenueRefusal`), `Response`, `AlreadyClosed` and `Capacity`. |
| B8 | low | rejected | The spec moves Binance's 2 server self-tests to the kit by name (Boundaries); `scripted.rs` is exercised by every suite that depends on its refusal, hold and ignore knobs (`a_refused_change_keeps_the_connection_delivering`, the edge tests, `client.rs`' `NoAnswer { id: 2 }`), as L1's B11 found for perps. |
| B9 | low | G8 patch | `Script::pushes`' doc says "after answering the first request" but `after_first_frame()` sends after the first frame whether or not it was answered; `received()` drops a non-JSON client frame silently (`.ok()?`). |
| B10 | low | G9 patch | deferred-work has no Story 4.11 entry for L2's kit changes, `Protocol::configure` and the owned `Outage<E>`, and L1's entry lists the kit's hooks without `configure`. |
| B11 | false | rejected | The read-back is an empty `Subscribe`, which sends nothing and answers at once, live or offline; it fails only when the path's task stopped, whose `Ended { stopped: true }` stops the whole client, so no caller is left with the stale view. |
| B12 | low | G10 patch | CLAUDE.md's "Tags decide" lists `UsdmWsError::from` as the wrap for a foreign error; it now takes a `WsError`, so a live test wraps in two steps. Patched under AD-21 rather than deferred: the commit that changed the rule should have edited it. |
| B13 | low | G11 patch | CLAUDE.md's shared-server paragraph says sports' server "goes with its other copies", though L2-12 deleted it. Patched under AD-21, as B12. |
| V1 | gap | G1 patch | Same as B1 (filed patch). |
| V2 | gap | G2 patch | No test changes a path reopened after a close for want of streams, so dropping `Router::ended`'s generation guard passes (filed patch). |
| V3 | gap | G3 patch | `DisconnectReason::Error` is asserted nowhere; `Outage::Error(_) => Stale` would pass (filed patch). |
| V4 | gap | G4 patch | Nothing pins that `RtdsBuilder::connect` honours `connect_timeout` on the first connect; reverting to `connect_to` (10 s) passes (filed patch). |
| V5 | gap | D2 defer | `UsdmWs::ping` keeping updates read meanwhile is unpinned; pre-existing, and the supervised tier does not use it (filed defer). |
| E1 | low | rejected | Same as B3. |
| E2 | low | rejected | A read that fails inside a change ends the outage as `Closed { None, "" }` rather than `Error`; L2-D1 accepts that the reason "may now read `Stale` or `Closed`", and the cause is logged at debug. |
| E3 | false | rejected | A superseded generation is one whose streams left; its task ends on `NotWanted` and `close()` answers `Ok`, so `stopped` is true only after a panic in the kit's task, which is a bug that ought to stop the client. |
| E4 | medium | D1 defer | A reconnect refused with 429 and a `Retry-After` longer than the backoff step retries inside the ban window: rtds newly (R1), Binance and perps as before the kit; honouring it is a cross-venue socket policy (D13), for Story 4.10's open `Retry-After` question. |
| E5 | low | G12 patch | `forward` returns on `events.closed()` and on a failed `reserve` without awaiting `Supervised::close`, so `SupervisedUsdmWs::close` drops its receiver first and can return before the polite closes go out; the old `close()` awaited each path's. |

## Design Notes

**The router.** `SupervisedUsdmWs` owns a router task, as today. The router:
- serves membership calls one at a time;
- holds at most one `Supervised<UsdmProtocol>` per path;
- forwards each path's items into one shared 1024-item queue. One option is a forwarder per path, with `Queue::Bounded(1)` on each path.

A path's stream ending after `close()` answers `Ok` means the path is no longer wanted: drop the path, and keep the merged stream running. An `Err` from `close()` means the path stopped: close every path, end the merged stream, and answer `Stopped`. A new path for the same `StreamPath` may open while the old one drains (`supervision_edges.rs:56`).

**`apply` for Binance**, as the old loop recorded it:
1. Check the cap: `TooManyStreams`, with nothing sent.
2. Record the change.
3. Send batches of 200, paced by the connection.
4. On `Refused`, undo the refused batch, keeping the batches already answered.
5. On a failed connection, return `Ok(())` and let the next read end it.

## Verification

**Commands:**
- `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: clean
- `cargo test -p polyoxide-binance --no-default-features --test headers` -- expected: pass
- `(cd .github/scripts && uv run pytest tests/ -q)`, `python3 scripts/gen_registry.py --check`, `python3 scripts/live_unwraps.py` -- expected: pass
- `python3 scripts/api_removals.py check` -- expected: green (delete `target/semver-checks` afterwards)
- the three live suites with `-- --ignored` -- expected: pass, or a tagged environmental or transient failure, recorded
