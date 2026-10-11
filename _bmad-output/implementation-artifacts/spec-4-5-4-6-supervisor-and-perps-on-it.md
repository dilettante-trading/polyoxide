---
title: 'Stories 4.5 and 4.6: the generic Supervisor, and perps on it'
type: 'feature'
created: '2026-10-10'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '397867758d8923666a75a7e40bc6a1ee20198493'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-4-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-4-1-4-4-socket-kit-classification-table-and-test-server.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** perps (`polyoxide-perps/src/ws/supervised.rs`, 1,132 lines) and Binance (`polyoxide-binance/src/usdm/ws/supervised.rs`) each run their own supervision task, with their own reconnect, keep-alive, staleness, membership replay and outage events (W10). Both block on a full event queue, which stops pings and makes blocked time count as silence; AD-11 forbids both. perps also lacks R1 (it gives up on a 408 or 425 refusal) and R2 (it has no connect timeout).

**Approach:** Add a generic `Supervisor<P: Protocol>` to `polyoxide-ws` (Story 4.5), proven by a unit suite against the shared scripted server. Then move perps onto it (Story 4.6) in A4-1's order:
1. switch perps' suites onto the shared server under the old loop;
2. move the loop onto the Supervisor, with the suites unchanged;
3. delete perps' loop, `Backoff` copy, TLS copy and `test_server`, and list what is removed.

Binance follows in bundle L2.

**Decisions (made as the user's delegate, 2026-10-10):**
- **L1-D1, Supervisor shape** (`polyoxide-ws`, module `supervisor`). This is the minimum; names below are binding, and other details are the implementer's choice, recorded in Implementation Notes.
  - **`Protocol`.** A generic-only trait (no dynosaur), whose async hooks return `impl Future + Send`.
    - **Associated types:** `Event`, `Error: From<WsError> + Send`, `Membership: Send`, `Change: Send` and `Ack: Send`.
    - **Hooks:** a handshake request built on every attempt; `replay` of the membership on a fresh connection (paced by the venue); `apply`, one membership `Change` on a live connection, answered with `Ack` or `Error`; `decode` of every frame (Text, Binary, Ping, Pong and Close) into events, a skippable error or nothing; liveness and delivered predicates; the ping to send at each ping tick (`None` sends none); and `wanted(&Membership)`, defaulting to true.
    - **Recovery and outages:** `recover(&Error) -> Recovery`, defaulting to `Reconnect` exactly when the venue's required `is_retriable(&Error)` says so; whether it declares `Disconnected`, and how it builds that event and `Reconnected`; and a per-connection reset hook.
    - **Its queue:** `Queue::{Bounded(n), Unbounded}`.
  - **`Recovery`** is `{ Reconnect, Skip, Stop }`. `Skip` yields the error and keeps the connection.
  - **`Supervised<P>`** is a `Stream<Item = Result<P::Event, P::Error>>`, with `membership() -> MembershipHandle<P>` and `close()`.
  - **`MembershipHandle<P>`** is `Clone`, with `apply(P::Change) -> Result<P::Ack, P::Error>`.
  - **Timings** come from `Timing`. The `polyoxide_ws` log target applies to the Supervisor's own logging.
- **L1-D2, the Supervisor's rules** (AD-11). The unit suite pins each one:
  - **Outage markers.** At most one `Disconnected` per outage, and only for a declaring `Protocol`; exactly one `Reconnected`, even when membership stops being wanted mid-outage.
  - **Pings and silence.** Pings stay on the wall clock while the consumer is slow: with the queue full, the Supervisor races `reserve()` against the ping timer and stops reading, and blocked time does not count as silence.
  - **No drops.** No decoded event is dropped.
  - **Not wanted.** `wanted` turning false on a healthy connection closes it politely, with no marker.
  - **Rotation.** A connection older than `max_connection_age` is rotated.
  - **Membership during an outage.** A change made during an outage is applied after the reconnect.
  - **Consumer gone.** Dropping `Supervised` ends the task promptly (Binance's `events.closed()` shape), and `close()` does not wait on a consumer that never read.
- **L1-D3, perps' consumer API:**
  - **Kept:** `PerpsWsBuilder`, which gains `connect_timeout` through `timing_setters!`, and its `connect(channels)`.
  - **New:** `PerpsProtocol` (public); `SupervisedPerpsWs` becomes `pub type SupervisedPerpsWs = polyoxide_ws::Supervised<PerpsProtocol>`; and a public extension trait `PerpsMembership` on `polyoxide_ws::MembershipHandle<PerpsProtocol>`, with `subscribe(Vec<Channel>)` and `unsubscribe(Vec<Channel>)` keeping today's results (`Refused`, `Stalled`, `Stopped`).
  - **`PerpsWsError::Connection`** now wraps `polyoxide_ws::WsError` (perps' part of Story 4.10, pulled forward because R2's timeout needs a home). Its `Classify` expands `polyoxide_venue::impl_ws_classification!()`, and Story 2.2's class rows are unchanged.
  - **Removed, each listed in `docs/s1-removals.md` with its replacement:**
    - the `ws::supervised` module;
    - `ws::MembershipHandle`;
    - `ws::Recovery` and `PerpsWsError::recovery()`;
    - the `SupervisedPerpsWs` struct, now an alias;
    - the doc-hidden `ws::test_server`, whose entry is marked `**Removed**`;
    - the `test-server` feature;
    - any other path the gate reports.
- **L1-D4, behaviour changes, each named in its commit and in the 4.11 release notes:**
  - **R1.** A handshake refused with 408, 425, 429 or 5xx is retried, by `retrying_can_fix` with venue's status rule.
  - **R2.** Connects time out at 10 s by default.
  - **AD-11.** Pings continue while the queue is full, replacing perps' documented blocking.
  - **Log target.** Supervision logs move to the `polyoxide_ws` target.
  - **Empty membership.** It stays wanted (SP:268): an empty membership reconnects and replays nothing, where today a reconnect with no channels ends the stream with `EmptySubscription`. `connect(vec![])` is still refused locally.
  - **Pings.** The ping is sent without awaiting its answer; an ok pong counts as liveness, and a pong that is not ok reconnects, as today.
- **L1-D5, the venue divergence rows perps keeps** (in `PerpsProtocol`):
  - **D4:** a JSON `post/ping` on the wall clock.
  - **D5:** liveness is frames plus an ok pong, never protocol Ping or Pong.
  - **D6:** delivered means a decoded frame.
  - **D10:** a `message_rate_limited` refusal is retried within `apply` up to `MAX_SUBSCRIBE_RETRIES` without reconnecting, and `SequenceRegressed` is emitted before its `Update`, per connection.
  - **D19:** `AlreadyClosed` stops the stream.
  - **D8:** perps declares no `Disconnected`.
  - **Queue:** bounded, 1024.

## Boundaries & Constraints

**Always:**
- **Test custody (AD-12).** A moved test keeps its name and assertions; its setup and calls may change. Report per-suite counts before and after: perps `ws/supervised.rs` 21 (17 supervision, 3 backoff, 1 defaults), `ws/client.rs` 9, `ws/error.rs` 10, and `test_server.rs` 5, whose 5 tests on perps' own answer move with it.
- **The order.** The old loop is deleted only once the suites pass against the Supervisor (A4-1), in separate commits.
- **CLAUDE.md** (AD-21) changes in the commit that changes the rule it states.
- **The removals gate.** `scripts/api_removals.py check` must be green before the bundle is done.
- **ws' dependencies.** Allowlists are regenerated only for a deliberate change, such as tokio's `sync` and `rt` for the Supervisor, and the commit says why.

**Never:**
- **No Binance, rtds, sports or clob change.** Their loops, servers and errors stay as they are; that is bundle L2 and later.
- No shims: no re-export of a removed path.
- The two backoffs are not merged (D13).
- No release, version bump or push.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected |
|---|---|---|
| Slow consumer | `Bounded(2)`, server pushes 10, consumer reads none for 3 ping periods | pings keep going out; no reconnect for staleness; all 10 events arrive in order once read |
| Outage, declaring | server drops, then accepts | exactly one `Disconnected` then one `Reconnected` |
| Outage, not declaring | the same, with a perps-like protocol | no `Disconnected`, one `Reconnected` |
| Unwanted mid-outage | membership emptied during the backoff; `wanted` false on empty | a declaring protocol yields `Reconnected` once; no further connect |
| Unwanted, healthy | membership emptied on a live connection | polite close, no marker, stream ends |
| Rotation | `max_connection_age` 200 ms | reconnects at age, replays membership |
| Change during outage | `apply` while reconnecting | applied after the reconnect; the caller gets its `Ack` |
| perps 503 / 408 refusal | refuse-upgrade on reconnect | retried (R1) |
| perps hung handshake | stalled upgrade | `ConnectTimeout` after `connect_timeout`, then retried |
| perps empty membership | unsubscribe every channel | the connection stays; a reconnect replays nothing; no `EmptySubscription` |
| perps `AlreadyClosed` | the hook sees it | `Stop` (D19) |
| Consumer drops `Supervised` | mid-stream | the task ends within one deadline tick |

</frozen-after-approval>

## Code Map

- **`polyoxide-ws`:**
  - **Available:** `Connection` (`src/bare.rs`; `Mode`, `last_inbound`, `closed`, `send`, `close`), its correlation (`src/request.rs`; `request`, `await_answer`, `Correlated`, `answer_timeout`, `min_request_interval`), `Timing`, `timing_setters!` and `next_wait` (`src/timing.rs`), `Backoff`, `connect`, `with_timeout` and `retrying_can_fix` (`src/connect.rs`), `WsError` and `Kind`, and `test_server` (`ScriptedServer::start` and `start_answering`, `Script`, the recorders, `wait_for`, and `helpers::{fast, next_item, next_event, drain, WINDOW}`).
  - **Gap:** tokio's normal features are only `net` and `time`; the Supervisor needs `sync` and `rt`. `ci/dep-allowlists/polyoxide-ws*.txt` pins the tree.
- **Basis loops:**
  - perps' `supervised.rs`: `run` `:303`, `pump` `:399`, `reconnect` `:358`, the ping `:412-432`, the wait `:435`, commands `biased` `:441` bounded by `stale_after` (`bounded` `:538`), `subscribe_with_retry` `:555`, `SequenceRegressed` `:508-521`, `close` `:273`.
  - Binance's `usdm/ws/supervised.rs`, for the generic needs L2 brings: the per-path router (`:466-657`), "start in outage" (`:719-735`), changes recorded during the backoff (`:834-855`), rotation (`:889`, polite close `:691`), `NotWanted` mid-outage still emitting `Reconnected` (`:767-769`, `:829-831`), and prompt drop via `events.closed()` (`:492`, `:912`).
  - Design the `Protocol` so Binance's rules fit without changing the Supervisor.
- **perps (`polyoxide-perps/src/ws/`):**
  - **`client.rs`:** `PerpsWs`, with `connect_to` `:78` (empty → `EmptySubscription`), `control` `:120-169`, `ping` `:173-189` and `poll_next` `:244-269`. It moves onto `polyoxide_ws::connect` plus `Connection` (`Mode::Data`); perps' decode skips Binary itself (D12, test `:438`). Ids start at 1 on both.
  - **`error.rs`:** `recovery` `:110-148`, `transport_class` `:162-176`, `Classify` `:187-209`. The table tests are `the_transport_follows_the_socket_table` `:325` and `every_variant_classifies` `:381`, both protected; the `recovery()` tests (`:225`, `:250`, `:261`, `:280`, `:287`, `:294`, `:316`) move to the `PerpsProtocol` hook.
  - **`supervised.rs`:** defaults `:26-35` (`MAX_SUBSCRIBE_RETRIES = 3`), and its tests `:591-1132`, with the local `fast` and `next_event` at `:591-610`.
  - **`test_server.rs`:** `Script` `:15`, `answer` `:156`.
  - **The shared server's equivalents:** `pushes` is `Script::frames` with `.after_first_frame()`; `raw_pushes` is `send`; `close_after` is `.then_close()`; `reject_handshake` is `Script::reject()`. `refuse`, `answer_pings`, `pong_status` and `answer_subscribes_after_first` become per-connection state in the answer closure, with the last config repeating. `subscriptions()` becomes `first_frames()`, and `wait_for` waits 5 s.
  - **The custody adapter.** A perps `#[cfg(test)]` adapter keeping the old `Script` field names is the cheapest way to keep test bodies.
- **Other users:**
  - `polyoxide-perps/tests/live_ws.rs:12`, `:58-201` (`PerpsWs`, the builder, `membership()`, `handle.subscribe`);
  - `polyoxide/src/lib.rs:118-119` re-exports `Channel`, `Event` and `PerpsWsBuilder`;
  - `docs/s1-removals.md:76` (`polyoxide_perps::ws::test_server`);
  - `.github/scripts/tests/test_classify_coverage.py:134`;
  - CLAUDE.md `:279-290` (the perps socket paragraph) and the kit paragraph ("no venue uses it yet").

## Tasks & Acceptance

**Execution** (one commit each, in order; run fmt, clippy and the touched tests before each):
- [x] **L1 (4.5).** Covers `polyoxide-ws/src/supervisor/` (new), `lib.rs`, `Cargo.toml` (tokio `sync` and `rt`) and the allowlists.
  - Add `Protocol`, `Recovery`, `Queue`, `Supervised` and `MembershipHandle` (L1-D1).
  - Add a unit suite against `ScriptedServer` with a test `Protocol` (declaring and not), covering every L1-D2 rule and the matrix's Supervisor rows.
- [x] **L2 (4.6, the server switch).** Covers `polyoxide-perps` (`Cargo.toml` dev-dependency on `polyoxide-ws` with `test-server`; the tests in `ws/client.rs`, `ws/supervised.rs` and `ws/error.rs`).
  - Point every perps suite at `polyoxide_ws::test_server`, through a `#[cfg(test)]` adapter carrying perps' answer.
  - perps' old loop and old server are still in place.
  - Record the counts before and after.
- [x] **L3 (DRIFT R2, R1).** Covers perps' `client.rs` and `error.rs`.
  - The bare tier connects through `polyoxide_ws::connect` with a timeout and runs on `Connection`.
  - `Connection` wraps `WsError` (L1-D3), and `Classify` uses the macro.
  - Its recovery retries 408 and 425 through `retrying_can_fix`.
  - Name R1 and R2 in the message, with a test for each.
- [x] **L4 (4.6, the loop).** Covers perps' `ws/protocol.rs` (new: `PerpsProtocol`, `PerpsMembership`), the builder's module and `mod.rs`.
  - `connect(channels)` builds `Supervised<PerpsProtocol>`.
  - The suites pass unchanged.
  - The L1-D4 behaviour changes are each covered by a test.
- [x] **L5 (4.6, deletion).**
  - Delete perps' `supervised.rs` loop, its `Backoff`, `ensure_crypto_provider`, `test_server.rs`, `Recovery` and the `test-server` feature, plus perps' direct tokio-tungstenite and rustls dependencies if nothing still uses them.
  - Add the `docs/s1-removals.md` entries (run the gate; copy each key).
  - Update `polyoxide-perps/README.md` if it names a removed path.
- [x] **L6 (docs).**
  - CLAUDE.md: the perps socket paragraph, the kit paragraph (perps on the Supervisor), and the `test_server` text.
  - CLAUDE.md's perps socket paragraph: say that the host's own 10 s pings and the client's automatic pongs count as traffic. K's probe found a `1011 pong_timeout` at about 70 s, but only with the pongs withheld.
  - `deferred-work.md`: Story 4.11 release-notes entries for L1-D3 and L1-D4.
  - `spine-amendments/epic-4.md`: an A4-4 entry for anything AD-11 did not settle that L1-D1 fixed: the `apply`/`Ack` membership path, `Recovery::Skip`, and `is_retriable` as a required hook.

**Acceptance Criteria:**
- **perps' suites.** Given perps' suites, when L4 lands, then every moved test passes with its name and assertions unchanged, and the counts reported in Implementation Notes match the ones before.
- **Removals.** Given `scripts/api_removals.py check` against `v0.38.1`, when L5 lands, then it is green, every removal is listed with its replacement, and the `test_server` entry is marked `**Removed**`.
- **The live suite.** Given `tests/live_ws.rs`, when it is compiled, then it builds against the new API; run it once with `-- --ignored` and record the result.
- **No other venue.** Given the other socket crates, when the bundle is done, then `git diff 3978677 -- polyoxide-binance polyoxide-rtds polyoxide-sports polyoxide-clob` is empty.

## Implementation Notes

**L1-D1, the details left to the implementer** (`polyoxide-ws/src/supervisor/`, re-exported at the crate root):
- **Construction.** `Supervisor::new(protocol, timing)`, then `connect(membership)`, which returns any failure of the first attempt, or `start(membership)`, which starts the stream in an outage when `recover` would retry that failure (Binance's start, for L2).
- **Hooks beyond the minimum.** `record(&mut Membership, Change) -> Offline<Change, Ack, Error>` decides a change made during an outage: `Recorded` answers at once and leaves it to the replay, `Held` (the default) waits for `apply` after the reconnect. `disconnected(Outage) -> Option<Event>` (default `None`) and `reconnected() -> Event` are the D8 markers; `Outage` is `#[non_exhaustive]` (`Closed`, `Error`, `Stale`, `Rotation`). `stalled(Duration)` and `stopped()` build the errors for a change left unanswered for `stale_after` and for a stopped task, by default `Kind::NoAnswer` and `Kind::AlreadyClosed`. `reset()` forgets per-connection state before a replay; `queue(&self) -> Queue`.
- **`Decoded`** is `Nothing`, `Event`, `Events` (for perps' `SequenceRegressed` ahead of its update) or `Error`, and `decode`, `is_live` and `delivered` see every frame (`Mode::All`).
- **`Error`** is bound by `std::error::Error` as well as `From<WsError> + Send`, so the task can log it.
- **tokio** gains `macros` as well as `sync` and `rt`, for `tokio::select!`; the allowlist gained `tokio-macros`, and `test-server` needs no tokio feature of its own any more.
- **The unit suite** (`src/supervisor/tests.rs`, 18 tests) drives a test protocol, declaring and not, against the shared server; three mutations were checked to fail it: counting the blocked time as silence, stopping the pings behind a full queue, and removing the consumer-gone branch.

**perps' protocol (L4):**
- **`PerpsProtocol` overrides `recover`.** perps' rule is not its class: a server close of any code, a `Capacity` failure and a pong that is not `ok` reconnect, a frame that does not decode is skipped, `AlreadyClosed` stops (D19), and a refused handshake follows venue's status rule through `retrying_can_fix` (DRIFT R1). The default (`Reconnect` iff `is_retriable`) would stop on a 1008 close, a `Capacity` failure and every decode failure. A4-4 records this against A4-3's follow-up.
- **D10's retry** is `retried_within_the_change` (every channel refused for `message_rate_limited`), inside `apply`, spaced by the backoff schedule. A refusal that reaches `recover`, from a replay, stops the stream, as the old loop's `Retry` did at a reconnect.
- **Pings.** `ping` takes an id from the connection and remembers up to 16 outstanding; `decode` matches a pong to one, and a pong that is not `ok` is `Response`, which reconnects. Any text frame is live (D5); delivered means a decoded push (D6).
- **`MembershipChange`** (`Subscribe`, `Unsubscribe`, `#[non_exhaustive]`) is public, since `MembershipHandle::apply` takes it; `PerpsMembership` wraps it.

**Where the code departs from the plan, and why:**
- **`ensure_crypto_provider` went in L3, not L5,** with its last caller, when the bare tier moved onto the kit's `connect`, which installs the provider.
- **The recovery tests moved to `ws/protocol.rs` as tests of `PerpsProtocol::recover`.** The kit's `Recovery` has no `Retry`, so the one test that asserted `Recovery::Retry` (`a_rate_limited_subscription_is_retriable_not_fatal`) asserts D10's predicate, `retried_within_the_change`, and the two that asserted `Fatal` for a refusal (`a_refused_subscription_lists_every_refusal`, `a_mixed_refusal_is_fatal`) assert `Stop` and that the change does not retry it, which `Fatal` implied. `SkipFrame` reads `Skip` and `Fatal` `Stop`.
- **CLAUDE.md's `test_server` text and the perps transport sentence changed in L5, not L6** (AD-21: the commit that changes the rule edits it). L4 had already rewritten the perps socket and kit paragraphs for the Supervisor. L6 changes only the idle rule, which no code commit changed.
- **perps' `ws` feature keeps only `tokio/time`.** The deleted loop used `sync`, `rt` and `macros`, and the deleted server `net`; perps' own code now calls only `tokio::time::sleep`, and `polyoxide-ws` enables what the kit needs.
- **`polyoxide_perps::ws::SupervisedPerpsWs` joined the type-alias list** in `docs/s1-removals.md`, since cargo-semver-checks 0.51.0 does not see aliases and the struct it replaced is listed as removed.
- **MSRV was not checked locally** (no rustup); CI's msrv job is the check.

**Test custody (AD-12).** `cargo test -p polyoxide-perps --features ws --lib`, per suite:

| Suite at 3978677 | Before | After |
|---|---|---|
| `ws/supervised.rs` | 21 (17 supervision, 3 backoff, 1 defaults) | 21 in `ws/builder.rs`, same names, plus 6 for L1-D4 |
| `ws/client.rs` | 9 | 9, plus 2 (R2, R3) |
| `ws/error.rs` | 10 | 3 stay, 7 (the `recovery()` tests) in `ws/protocol.rs`; plus 1 in `error.rs` (the 451 non-fault) and 2 in `protocol.rs` (R1, the kit's own failures) |
| `ws/test_server.rs` | 5 | 5 in `ws/scripted.rs`, same names |
| perps lib, all | 81 | 92 |

All 45 protected names are present after L5 (checked by name against 3978677). L2 recorded 81 before and after the server switch; L4 reports each moved supervision test also passing on the old loop.

**The removals gate (L5).** `scripts/api_removals.py check --baseline v0.38.1`, run locally on rustc 1.95.0 with cargo-semver-checks 0.51.0: the first run reported ten unlisted perps keys (`ws::supervised`, `ws::supervised::PerpsWsBuilder`, the `SupervisedPerpsWs` struct at both paths, `MembershipHandle` at both, `Recovery` at both, `PerpsWsError::recovery` and the `test-server` feature); with each listed, 43 removals, 43 listed, and every doc-hidden and alias path compiles, the new `SupervisedPerpsWs` alias among them.

**The live suite**, `cargo test -p polyoxide-perps --features ws --test live_ws -- --ignored`, 2026-10-11 08:10 UTC: `live_all_tickers_fans_out_per_instrument_and_membership_changes_apply`, which drives the supervised tier and its membership calls, passed. `live_bare_stream_delivers_book_and_bbo_frames_and_answers_ping` failed tagged `environmental` in five of six runs and passed in one. Instrumented, the bare tier yielded 154 `book` frames on instrument 1 in 15 s and no `bbo` frame; an independent Python client on the same host saw `book::1` keep one best bid and ask for 20 s and `bbo::1` push nothing, while `bbo::6` to `bbo::10` pushed. The host pushes `bbo` only when the top moves, and the test picks the first two-sided book, so this is the market, not the bundle. Recorded in `docs/specs/perps/OBSERVED.md`, with the selection fix deferred.

## Spec Change Log

## Review Triage Log

Review 1, 2026-10-11: three layers (blind B, edge-case E, verification-gap V and its other findings VO), 39 findings. Groups: G1–G16 patched, D1–D3 deferred, the rest rejected.

| # | Verdict | Group | Evidence |
|---|---|---|---|
| B1 | low | G3 patch | `Task::apply` and the held applies are awaited outside the ping timer, so no ping goes out for up to `stale_after` while a change waits; the old perps loop did the same, and perps' host keep-alive is its protocol Ping, which the request's reads still answer. The harm is the rustdoc saying "whatever the traffic". |
| B2 | medium | G2 patch | `pump` checks rotation and staleness at the top of its loop, before `connection.next()` drains the frames a request kept, so a long `apply` can end the connection as stale with pushes still buffered, and they are lost (the old loop drained them first). The remaining part, that the answer itself never counts as liveness, needs a server that takes most of a window to answer on a quiet connection; rejected as low. |
| B3 | maybe-false | D2 defer | No protocol in this build sets an answer timeout, so no `NoAnswer` reaches `Task::apply`. Whether Binance's 10 s answer timeout fits W12 without a Supervisor change (close in `apply`, or use the `stale_after` bound) is Story 4.7's to settle; medium if it does not. |
| B4 | low | G4 patch | `End::Rotation` sets `next_delay` without `backoff.after_connection_ended(delivered)`, so a failed attempt after a rotation waits from the last outage's schedule. |
| B5 | medium | G5 patch | `Task::apply` awaits `protocol.apply` under `timeout(stale_after)` without racing `events.closed()`, so a drop or `close()` with a change pending waits up to `stale_after`, against L1-D2's "ends the task promptly". |
| B6 | low | G6 patch | No test asserts the stream keeps delivering on the same connection after `apply` returns a refusal; routing apply errors through `recover` (Refused → Stop) would pass every test. |
| B7 | medium | D1 defer | A replay refused with `message_rate_limited` stops the stream; the old loop's `reconnect` returned the same refusal as fatal, so the rule is pre-existing. The custody note: the test keeps its name and asserts D10's predicate. |
| B8 | low | G7 patch | A4-4 says `apply` records "as much of it as took effect", but perps records a change only when every channel was accepted (as the old loop did), and `PerpsMembership::unsubscribe`'s doc does not say what a partly refused unsubscribe leaves. The re-sent accepted channels on a rate-limit retry are the old `subscribe_with_retry`'s too. |
| B9 | low | G8 patch | `Recovery`, `Queue`, `Decoded` and `Offline` are public kit enums without `#[non_exhaustive]`; venues only construct them, so the attribute costs nothing now and keeps a later variant from being a break. |
| B10 | low | G9 patch | `supervisor/mod.rs`'s basis note cites perps' `ws/supervised.rs` and `test_server/mod.rs`'s cites perps' `ws/test_server.rs`, both deleted here; `timing.rs`'s D5 still gives perps' old "frames and an ok pong". |
| B11 | low | rejected | `as_perps_will_use_it` in `polyoxide-ws/src/test_server/tests.rs` tests the shared server's per-connection answering with an example answerer; it runs nothing of perps', so drift cannot hide a perps defect. Cosmetic. |
| B12 | medium | G10 patch | `polyoxide::prelude` re-exports `PerpsWsBuilder` under `perps-ws` but not `PerpsMembership`, so a prelude user's `membership().subscribe(..)` stops compiling; the Code Map names `polyoxide/src/lib.rs:118-119` as a user to update. |
| B13 | false | rejected | `epic-4-context.md` is the compiled context bundle L1 planned from (da05409); it describes the state at compile time, and the next build recompiles it, since `spine-amendments/epic-4.md` is now newer. |
| B14 | low | G11 patch | `PerpsWsError::Stalled`'s doc says only "No frame arrived"; it also reports a change or a replay left unanswered for the window. |
| B15 | false | rejected | `polyoxide-ws/src/bare.rs` frames D12 as the Binary-frame row ("clob decodes it as text, and the others skip it"), so citing D12 for perps' skip is consistent. |
| B16 | low | rejected | The bare tier's `Kind::Closed` → `ConnectionClosed` mapping keeps the old `await_response` result on purpose ("as before the kit", L1-D3); a bare-tier connect-timeout knob would be new public API nobody asked for. |
| B17 | low | G12 patch | The full-queue 1011 risk is in CLAUDE.md and deferred-work but not in `SupervisedPerpsWs`'s rustdoc, which users read. |
| V1 | gap | G13 patch | `PerpsWsBuilder::connect([])`'s `EmptySubscription` has no test through the builder (filed patch). |
| V2 | gap | G14 patch | The `stale_after` bound on `replay` in `attempt` has no test (filed patch). |
| V3 | gap | G15 patch | Nothing tests that perps' `is_live` ignores protocol Pings under the Supervisor (filed patch). |
| V4 | gap | G16 patch | Nothing tests that `reset` clears `last_sq` per connection (filed patch). |
| V5 | gap | D3 defer | Backoff reset after a delivering connection is untested, as it was in the old loop; a timing test needs care (filed defer). |
| VO1 | medium | G1 patch | Same as E1. |
| VO2 | medium | G10 patch | Same as B12. |
| E1 | medium | G1 patch | A read that fails inside a request (EOF without a close) marks the connection finished and is answered to the caller; if a ping falls due before the next read, `Connection::send` reaches tungstenite in `Terminated`, which returns `AlreadyClosed` (0.26.2 `check_not_terminated`), and perps' D19 rule stops the stream instead of reconnecting. |
| E2 | low | G1 patch | A close frame read inside a request, then a due ping, gives `SendAfterClosing`, so the outage is `Outage::Error` rather than `Outage::Closed` and the close code is lost; same root cause as E1. |
| E3 | medium | G2 patch | Same as B2's buffered-frames part, for rotation as well as staleness. |
| E4 | low | G3 patch | Same as B1. |
| E5 | low | G4 patch | Same as B4. |
| E6 | low | rejected | A zero `ping_interval` pings on every pass; the old perps loop did too, `Duration::MAX` is the documented "never", and perps exposes no `max_connection_age`; a clamp adds a guard. |
| E7 | false | rejected | `Queue::Bounded` is set by a venue's protocol (perps 1024), not by user input; a bound over tokio's permit limit panics loudly in development. |
| E8 | low | rejected | perps' membership is always wanted, and no protocol in this build connects with an unwanted one; Story 4.7's builder refuses an empty start. |
| E9 | low | rejected | A change whose caller cancelled after handing it over is still applied, with the server and the membership agreeing; the old loop did the same, and the cancellation is racy either way. |
| E10 | low | rejected | A panicking hook ends the stream with `None` and `close()` returns `stopped`; the old loop ended the same way, and a hook that panics is a bug. |
| E11 | medium | D1 defer | Same as B7. |
| E12 | low | G7 patch | A partly refused unsubscribe leaves the channels the server dropped in the membership, so a reconnect re-subscribes them; pre-existing (the old loop updated `channels` only on full success), and L1-D3 keeps today's results, so only the docs change. |
| E13 | medium | G10 patch | Same as B12. |
| E14 | medium | G5 patch | Same as B5. |
| E15 | false | rejected | A fatal end of an outage yields its error and ends the stream, as Binance's supervised tier documents ("yielded as `Err` and then the stream ends"); `Reconnected` follows `Disconnected` while the client runs. |

## Design Notes

A sketch of the trait's shape, not binding beyond L1-D1:

```rust
pub trait Protocol: Send + Sized + 'static {
    type Event: Send + 'static;
    type Error: From<WsError> + Send + 'static;
    type Membership: Send + 'static;
    type Change: Send + 'static;
    type Ack: Send + 'static;
    fn handshake(&mut self) -> Result<Request, Self::Error>;          // every attempt
    fn replay(&mut self, conn: &mut Connection, m: &Self::Membership)
        -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn apply(&mut self, conn: &mut Connection, m: &mut Self::Membership, c: Self::Change)
        -> impl Future<Output = Result<Self::Ack, Self::Error>> + Send;
    fn decode(&mut self, frame: Message) -> Decoded<Self::Event, Self::Error>;
    fn ping(&mut self) -> Option<Message>;
    fn is_retriable(err: &Self::Error) -> bool;
    fn recover(&self, err: &Self::Error) -> Recovery { /* Reconnect iff is_retriable */ }
    fn wanted(m: &Self::Membership) -> bool { true }
    // liveness / delivered predicates, outage events, queue, per-connection reset …
}
```

`apply` and `replay` borrow the `Connection`, so they can use its request correlation. A request is not cancel-safe, so the Supervisor bounds them with `stale_after`, and on a timeout it replaces the connection rather than reusing it.

## Verification

**Commands:**
- `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: clean
- `(cd .github/scripts && uv run pytest tests/ -q)`, `python3 scripts/gen_registry.py --check`, `python3 scripts/live_unwraps.py` -- expected: pass
- `python3 scripts/api_removals.py check` -- expected: green (about 8.5 min cold; delete `target/semver-checks` afterwards)
- `cargo test -p polyoxide-perps --features ws --test live_ws -- --ignored` -- expected: pass, or a tagged environmental/transient failure, recorded
