---
title: 'Stories 4.5 and 4.6: the generic Supervisor, and perps on it'
type: 'feature'
created: '2026-10-10'
status: 'ready-for-dev'
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
- [ ] **L1 (4.5).** Covers `polyoxide-ws/src/supervisor/` (new), `lib.rs`, `Cargo.toml` (tokio `sync` and `rt`) and the allowlists.
  - Add `Protocol`, `Recovery`, `Queue`, `Supervised` and `MembershipHandle` (L1-D1).
  - Add a unit suite against `ScriptedServer` with a test `Protocol` (declaring and not), covering every L1-D2 rule and the matrix's Supervisor rows.
- [ ] **L2 (4.6, the server switch).** Covers `polyoxide-perps` (`Cargo.toml` dev-dependency on `polyoxide-ws` with `test-server`; the tests in `ws/client.rs`, `ws/supervised.rs` and `ws/error.rs`).
  - Point every perps suite at `polyoxide_ws::test_server`, through a `#[cfg(test)]` adapter carrying perps' answer.
  - perps' old loop and old server are still in place.
  - Record the counts before and after.
- [ ] **L3 (DRIFT R2, R1).** Covers perps' `client.rs` and `error.rs`.
  - The bare tier connects through `polyoxide_ws::connect` with a timeout and runs on `Connection`.
  - `Connection` wraps `WsError` (L1-D3), and `Classify` uses the macro.
  - Its recovery retries 408 and 425 through `retrying_can_fix`.
  - Name R1 and R2 in the message, with a test for each.
- [ ] **L4 (4.6, the loop).** Covers perps' `ws/protocol.rs` (new: `PerpsProtocol`, `PerpsMembership`), the builder's module and `mod.rs`.
  - `connect(channels)` builds `Supervised<PerpsProtocol>`.
  - The suites pass unchanged.
  - The L1-D4 behaviour changes are each covered by a test.
- [ ] **L5 (4.6, deletion).**
  - Delete perps' `supervised.rs` loop, its `Backoff`, `ensure_crypto_provider`, `test_server.rs`, `Recovery` and the `test-server` feature, plus perps' direct tokio-tungstenite and rustls dependencies if nothing still uses them.
  - Add the `docs/s1-removals.md` entries (run the gate; copy each key).
  - Update `polyoxide-perps/README.md` if it names a removed path.
- [ ] **L6 (docs).**
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

## Spec Change Log

## Review Triage Log

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
