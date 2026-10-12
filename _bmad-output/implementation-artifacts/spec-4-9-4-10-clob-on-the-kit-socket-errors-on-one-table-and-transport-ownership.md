---
title: 'Stories 4.9 and 4.10: clob on the kit, every socket error on one table, the kit owns the transport'
type: 'feature'
created: '2026-10-12'
status: 'in-progress'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'e1144543ec120d4597b4c9fff674120d554d3ae9'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-4-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-4-7-4-8-binance-on-the-supervisor-rtds-and-sports-on-the-kit.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** clob's sockets (`polyoxide-clob/src/ws/client.rs`, 1,228 lines) still carry their own TLS install, a raw `tokio-tungstenite` read path with no close reply (R3), a boxed tungstenite error with its own class table, two ad-hoc loopback test servers, and a live suite that installs TLS itself (R9). sports still boxes tungstenite's error and reads a raw socket. clob's and sports' 451 refusals count as faults, and no socket error reports a refusal's `Retry-After` the way HTTP errors do. A reconnect refused with a long `Retry-After` retries inside the ban window (deferred E4). Binance's `Stopped` drops its cause, so a stream stopped by a 451 tags `real`. Nothing stops a crate from declaring `tokio-tungstenite` or `rustls` again.

**Approach:** In A4-1's order:
- **clob (4.9).** Switch its suites onto `polyoxide_ws::test_server` under the old code. Then read through the kit's `Connection`, keeping clob's own per-address connect. Then delete its copies.
- **The other socket crates (4.10).** sports moves onto the kit's blocks and the socket table. Every socket error surfaces `Retry-After`, and every reconnect waits it out, under a ceiling. Binance's `Stopped` keeps its cause. Two CI fences hold the result.

**Decisions (made as the user's delegate, 2026-10-12):**
- **M-D1, clob reads through `Connection`.**
  - `WebSocket` and `WebSocketWithPing` hold a `polyoxide_ws::Connection` in `Mode::Data`, built over clob's own connect.
  - D12 stays at clob's call sites: a Binary frame is decoded as text.
  - clob keeps its own unbiased `select!`, its 10 s text `PING` and its outbound queue. It sends through `Connection::send`.
  - The venue acknowledges nothing, so clob does no request correlation. Public shapes are unchanged except `WebSocketError` (M-D3).
- **M-D2, clob's connect stays its own (new divergence row D20).** It keeps the per-address Happy Eyeballs loop.
  - **Timeouts.** The TCP connect and the TLS-plus-upgrade are each bounded by `with_timeout`, so `connect_timeout` keeps meaning "per address, per step". `Kind::ConnectTimeout` maps back to `ConnectTimeout { url, timeout }`.
  - **The upgrade** runs through a new kit function, `polyoxide_ws::connect_over(request, tcp: TcpStream) -> Result<Socket, WsError>`. It installs TLS first.
  - **DNS.** "No addresses resolved" becomes `WsError::from(tungstenite::Error::Io(NotFound))`, so it is classed `Network` (deferred entry :35).
- **M-D3, clob's error, its share of 4.10, as perps' and rtds' were.**
  - `WebSocketError::Connection(#[from] WsError)`. The type expands `impl_ws_classification!`, with `is_fault` from `ws_is_fault`.
  - `From<tungstenite::Error>` goes, and the enum becomes `#[non_exhaustive]`.
  - `ConnectionClosed` and `Authentication` are never constructed, but they stay: S2 renames the enum and may prune them then.
- **M-D4, sports on the blocks.**
  - **Error.** `SportsError::{Connect, Transport}` carry `WsError`, with their variant names kept. `ConnectTimeout` and `Closed` are mapped from `Kind`, as Binance's are, so their text is unchanged. The macro classifies, with `is_fault`, and `transport_class` is deleted.
  - **Connect and read.** `open` uses `polyoxide_ws::connect`. Both tiers read a `Connection`, which brings the kit's close reply. Pings still reset staleness (D5), and both tiers stay `Send + Unpin`.
  - **Recovery** is a private function in `supervised.rs`, because D7 keeps sports off the Supervisor. `Kind::Capacity` reconnects, `Kind::AlreadyClosed` reconnects (D19, now pinned by a test), and anything else follows `retrying_can_fix` with venue's rule. The recovery tests move with it by name.
  - sports declares no `tokio-tungstenite`.
- **M-D5, rtds keeps its public `Recovery` and `recovery()`** (L2-D6) as its recovery hook. Moving them would cost three removals for no behaviour change.
  - So AC (b) is met in three ways: by the perps and Binance `Protocol`s, by sports' private function, and by rtds' hook.
  - D19's wording, "through its `Protocol` recovery hook", is fixed to match.
- **M-D6, sockets surface `Retry-After` as HTTP does.**
  - The class carries a 429's wait. `retry_after()` returns any refusal's `Retry-After`, unclamped, and a zero is `None` (R4).
  - The macro gains a private `ws_retry_after`. Every socket `Classify` uses it, so a venue that forgets fails `-D warnings`.
- **M-D7, DRIFT R11: a reconnect after a refusal waits out its `Retry-After`.** This applies core's rule to sockets: `Retry-After` only extends the wait, never shortens it, and it is clamped to the venue's `Backoff` ceiling.
  - **The kit** gains a `Protocol::retry_after(&self, &Self::Error) -> Option<Duration>` hook, `None` by default, and the supervisor waits `max(step, hook.min(ceiling))`. perps and Binance answer the hook from `Classify`.
  - **rtds and sports** do the same at their call sites.
  - **clob** never reconnects, so it is untouched.
- **M-D8, Binance's `Stopped` keeps its cause, Binance-local (no kit change).**
  - The router snapshots the error that stopped a path: its class, `is_fault`, `retry_after` and text. `Stopped` carries that snapshot as an `Option`.
  - `Classify` follows the snapshot. A `Stopped` with no cause stays `InvalidRequest` and a fault.
  - `close()` and every later membership call carry the cause.
  - perps' identical gap gets a deferred entry.
- **M-D9, the fences.**
  - **Dependencies.** `test_dependency_fences.py` generalises the reqwest fence to an owner table: `reqwest` is `polyoxide-core`'s, and `tokio-tungstenite` and `rustls` are `polyoxide-ws`'s. It covers every dependency kind, and each owner must declare its crate, so the check can never pass vacuously.
  - **The macro.** `test_classify_coverage.py` fails when a type with a `WsError` field sits in a file that does not expand `impl_ws_classification!`. It gets a non-vacuity check and a synthetic test.
- **M-D10, custody exceptions.** Only the assertion text changes; every tag is unchanged.
  - **sports:**
    - `a_reconnect_refused_for_good_ends_the_stream` matches on `kind()`;
    - `failure_tags::reset_without_closing_handshake` updates its Debug text, with its Python row.
  - **clob:** the `error.rs` rows are built through `WsError`, which changes how they are constructed, not what they assert.
  - **Binance:** the `Err(UsdmWsError::Stopped)` patterns become `Stopped { .. }`.
  - **Moved:** clob's `a_default_crypto_provider_is_available_to_connect` goes to the kit's `tls.rs` tests by name.
- **M-D11, the behaviour changes.** Each is named in its commit and in 4.11's release notes:
  - **clob:**
    - R3;
    - `close()` returns `Ok` after a server close;
    - the stream ends after its first error;
    - a DNS miss is `Network`, with new text;
    - a 451 is not a fault;
    - a 429 carries its wait;
    - the Debug text changes;
    - `From<tungstenite::Error>` is gone;
    - the enum is `#[non_exhaustive]`;
    - clob publishes after `polyoxide-ws`.
  - **sports:** a 451 is not a fault, the `Retry-After` is surfaced, and the Debug text changes.
  - **All reconnecting venues:** R11.
  - **Binance:** the shape of `Stopped`.
  - **The kit:** `connect_over`, `Protocol::retry_after` and the `Backoff` floor.

## Boundaries & Constraints

**Always:**
- **Test custody (AD-12).** A moved test keeps its name and assertions; M-D10 lists the only exceptions. Per-suite counts are reported before and after, from `cargo test -- --list`.
  - **Counts at e114454:**
    - clob inline ws: `client.rs` 20 (connect 4, crypto 1, tests 6, dispatch 4, membership 5), `error.rs` 2, `auth.rs` 8, `market.rs` 20, `subscription.rs` 21, `user.rs` 21; `live_ws` 2;
    - sports: `supervision.rs` 15, `bare.rs` 6, `failure_tags.rs` 7, inline `supervised.rs` 6, `client.rs` 7, `error.rs` 10;
    - Binance: `supervision.rs` 21, `supervision_edges.rs` 7, `supervision_kit.rs` 10, `protocol.rs` 6, `error.rs` 5, `failure_tags.rs` 17;
    - `polyoxide-venue/tests/ws_classification.rs` 6;
    - `.github/scripts` 927.
- **The order.** The server switch comes first, then the move, then the deletion, in separate commits (A4-1). A DRIFT row gets its own commit and a test named for it.
- **Docs.** CLAUDE.md changes in the commit that changes the rule (AD-21). Generated regions come from `gen_registry.py --write`, in the same commit.
- **Gates.** `scripts/api_removals.py check` is green. The rtds and sports allowlists pass unchanged. clob, perps, Binance, rtds and sports declare neither `tokio-tungstenite` nor `rustls`.

**Never:**
- No change to `polyoxide-core`.
- No clob `Supervisor`: that is Epic 7.
- rtds and sports stay off the Supervisor (D7).
- No shims and no re-exports of a removed path.
- The socket `Backoff` stays separate from core's backoff (D13).
- rtds' `Recovery` stays public.
- No release, no version bump, no push.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected |
|---|---|---|
| clob server close | the server closes during `run` or the stream | a close reply is sent (R3); `run` returns `Ok`; the stream ends |
| clob Binary frame | an event sent as Binary | decoded as text (D12) |
| clob DNS miss | a host with no addresses | `Connection` classed `Network` |
| clob hung handshake | a stall | `ConnectTimeout { url, timeout }` after one step's timeout |
| clob or sports 451 | the upgrade is refused with 451 | `Restricted`, not a fault |
| any socket 429 or 503 with `Retry-After: 30` | the upgrade is refused | `retry_after()` is 30 s; only the 429's class carries it |
| reconnect refused with `Retry-After` | a backoff step of 1 s, a 30 s wait, a 60 s ceiling | waits 30 s (R11); with a 600 s wait, it waits the 60 s ceiling |
| sports `Capacity` or `AlreadyClosed` | either error | reconnect |
| Binance path stopped by a 451 | a reconnect refused with 451 | `close()` returns `Stopped`, classed `Restricted`, not a fault |
| fence: direct `rustls` | a non-ws member declares it, in any kind | the test fails, naming `polyoxide-ws` |
| fence: a `WsError` field | the type's file has no macro | the test fails |

</frozen-after-approval>

## Code Map

- **Environment.** `/tb` runs near full and earlyoom SIGTERMs rustc: build with `CARGO_INCREMENTAL=0`; a rustc killed by signal 15 (exit 254) is environmental, so re-run it. Another loom session builds in a sibling worktree at the same time. Delete `target/semver-checks` after each removal-gate run. The `divergences.md` part of M-10 is the lead's, through `bmad-spec`; the implementer leaves that file alone.
- **The kit (`polyoxide-ws/src/`)**
  - `connect.rs`: `connect` :34, `with_timeout` :49, `retrying_can_fix` :76. `connect_over` goes here.
  - `bare.rs`: `Connection::new(Socket, Mode)` :86, which accepts any pre-connected socket; `last_inbound` :107; `closed` :117; `send` :124; `close` :133; the R3 flush in `poll_next` :204-258.
  - `error.rs`: the `WsError` constructors :107-130; `From<tungstenite::Error>` :169. There is no I/O constructor, so build one through `polyoxide_ws::tungstenite::Error::Io`.
  - `backoff.rs`: `take` :70. Add a floor that clamps to `max`.
  - `supervisor/mod.rs`: the `Protocol` hooks :130-256, and `stopped()` :254, which takes no argument. `task.rs`: `reconnect` :397-445, with the wait at :439, and the first-delay path :298.
  - **Stale text to rewrite:**
    - `tls.rs:17-21`, which says clob's copy stays;
    - `error.rs:20-26` (clob and sports box tungstenite);
    - `connect.rs:29-33,72-75` (sports' own rule);
    - `bare.rs:52-59`, which needs clob added;
    - `test_server/mod.rs:5-14`, which may add clob's two servers.
- **clob (`polyoxide-clob/src/ws/`)**
  - `client.rs`:
    - TLS copy :32-52; `connect_ws` :84-129, with the DNS miss :104 and the two timeouts :106-125;
    - `poll_next` :483-512 (D12 :492-502, a close with no reply :504);
    - `run` :782-834, with the `PING` arm :789-799 and the close :827;
    - the private `WebSocketWithPing::new` :719.
  - **Tests in `client.rs`.** `membership_tests` :1057-1228 hold `recording_server` :1070 and `ping_counting_server` :1120, and five tests. On the scripted server:
    - **Holding open:** `Script::hold()`.
    - **Counting pings:** `count_frames(|t| t == "PING")`.
    - **Recording:** `wait_for` on the non-`PING` `client_frames`.
    - **Ending a run:** `drop_after` or `close_at_once`. Don't use `after_first_frame`: the `select!` is unbiased, so the first frame may be a `PING`.
    - **Connecting:** go through the in-module `connect_ws`, since the builder refuses `ws://`.
  - `error.rs`: the variants :6-47, `From` :49-53, `transport_class` :58-72, `Classify` :77-89; tests at :102 and :152.
  - `lib.rs:200-206` holds the classify assertion.
  - `tests/live_ws.rs`:
    - **R9 site** :97. Open the probe through `polyoxide_ws::connect`, as `polyoxide-binance/tests/live_ws.rs:152` does.
    - **Error builds** at :101, :106 and :140.
  - **`Cargo.toml`:**
    - the `ws` feature :16-23;
    - `tokio-tungstenite` :46 and `rustls` :52, with the comment at :48-51, which go;
    - add `polyoxide-ws` as an optional dependency under `ws`, and as a path dev-dependency with `test-server`.
  - **Docs:** `README.md:30`'s feature row.
  - **Consumers.**
    - The CLI's `ws market` and `ws user` call `close().await?` (`market.rs:165`, `user.rs:166`); their code is unchanged.
    - The umbrella's `pub use polyoxide_clob::ws` (`polyoxide/src/lib.rs:107`) is unchanged.
- **sports (`polyoxide-sports/src/`)**
  - `client.rs`: `open` :27-38; the bare `poll_next` :147-182, with its own close flush :175.
  - `supervised.rs`:
    - `poll_reading` :280-327 (D5), with its close flush :322;
    - the recovery call :264;
    - the backoff wait :270, which is an R11 site.
  - `error.rs`:
    - the variants :30-75;
    - `retrying_can_fix` :85-102, which is `pub(crate)` and moves;
    - `transport_class` :108-122;
    - `Classify` :127-136;
    - the protected tests :184-350.
  - **Tests:**
    - `tests/supervision.rs:283-286` (M-D10);
    - `tests/failure_tags.rs:30-41`;
    - `tests/live_api.rs:34-37,58-91` (`polyoxide_ws::Socket` and `connect`);
    - `tests/bare.rs:9` and `supervision.rs:13`, whose imports change to `polyoxide_ws::tungstenite`.
  - `Cargo.toml:33-37`: drop `tokio-tungstenite`.
  - The CLI matches `Stale` and `Decode` (`polyoxide-cli/src/commands/ws/sports.rs:139`), and so does the README doctest (`README.md:36`). Keep both.
- **Binance (`polyoxide-binance/src/usdm/ws/`)**
  - `error.rs`: `Stopped` :97-99, `Classify` :132-154.
  - `supervised.rs`: `MembershipHandle::send` :304-311, `SupervisedUsdmWs::close` :379-394, `Router::ended` :465-479 and `forward` :660-695. `forward` sees the `Err` item before `close()`, at :678 and :684-690.
  - `protocol.rs`: `recover` :260-288, which gains the R11 hook; `stopped` :322-324.
  - **Patterns** at `tests/supervision.rs:674` and `tests/supervision_kit.rs:417`.
- **perps:** `ws/protocol.rs` `recover` :279-312 gains the R11 hook. `ws/error.rs` `Classify` :112-138 gains `retry_after`.
- **rtds:** `error.rs` `Classify` :185-201 gains `retry_after`. The R11 wait goes at `supervisor.rs:210`.
- **The venue crate:**
  - `polyoxide-venue/src/ws_table.rs:79-117` (the macro) gains `ws_retry_after`;
  - `polyoxide-venue/tests/ws_classification.rs` gains rows for it.
- **CI:**
  - `.github/scripts/tests/test_dependency_fences.py`: the reqwest fence :67-79 and :119-154 becomes the owner table.
  - `test_classify_coverage.py`: the `WsError` exemption :43-48 and the sweep tests :130-156.
  - `test_classify_failures.py:331`: the Python row for sports' reset.
- **Docs and planning:**
  - **`deferred-work.md`:**
    - close :34-39, :44, :177, :220 and :229;
    - add 4.11 entries for M-D11;
    - add a deferred entry for perps' `Stopped`.
  - **`docs/s1-removals.md`:** cargo-semver-checks reports no removal for a removed `From` impl or a changed field type (rtds precedent). List only what the gate reports.
  - **CLAUDE.md:** :50, :180, :342, :471, :511, :528, :530, :540 and :544, plus the generated :76 and :558-560.
  - **`_bmad-output/specs/spec-venue-extensibility/divergences.md`, through `bmad-spec` (lead):**
    - add D20 (clob's connect) and R11;
    - fix D19's and D10's wording;
    - update the R3 cite (`client.rs:504`), the R9 cite (now `live_ws.rs:97`) and the R1 cite (`error.rs:85`).

## Tasks & Acceptance

**Execution.** One commit each, in order. Run fmt, clippy and the touched tests before each commit.
- [ ] **M-1 (4.9, the server switch).** clob's five `membership_tests` run on `polyoxide_ws::test_server` under the old code. Add the dev-dependency, and record the counts before and after.
- [ ] **M-2 (the kit).** Add `connect_over`, with a test against the scripted server, and move clob's crypto test into `tls.rs` by name.
- [ ] **M-3 (4.9, the move, DRIFT R3).** clob makes the M-D1 to M-D3 changes, with tests:
  - `a_server_close_is_answered_with_the_close_reply_drift_r3`;
  - the DNS-miss row;
  - the 451 and 429 rows;
  - a per-step `ConnectTimeout` on `Script::stall()`.
- [ ] **M-4 (4.9, deletion).**
  - Delete the TLS copy and the old read path.
  - Move `live_ws` to R9.
  - Drop `tokio-tungstenite` and `rustls`.
  - Update Cargo, run `gen_registry --write`, and update the README, CLAUDE.md, the kit basis notes and `s1-removals`.
- [ ] **M-5 (4.10, sports).** Make the M-D4 changes, with the M-D10 exceptions, plus the 451 and 429 rows and a D19 pin test. Drop `tokio-tungstenite`.
- [ ] **M-6 (4.10, `Retry-After`).** Add the macro's `ws_retry_after` and the five `Classify` overrides (M-D6), with venue and per-crate rows.
- [ ] **M-7 (DRIFT R11).** Add the kit hook and the `Backoff` floor, used by perps, Binance, rtds and sports. Each gets a test named `..._drift_r11`.
- [ ] **M-8 (4.10, Binance's `Stopped` cause).** Make the M-D8 changes, with a `supervision_kit.rs` test and a failure-tag twin.
- [ ] **M-9 (4.10, fences).** The M-D9 fences, each shown failing on a synthetic case, plus CLAUDE.md :50 and :342.
- [ ] **M-10 (docs).**
  - The `deferred-work.md` entries.
  - The lead's `bmad-spec` update of `divergences.md`.
  - The remaining CLAUDE.md lines.

**Acceptance Criteria:**
- **Custody.** Given the suites in Boundaries, when M-4 and M-5 land, then every moved test passes with its name, and with its assertions unchanged except under M-D10, and the counts reconcile.
- **Transport ownership.** Given `cargo metadata`, when the fence runs, then only `polyoxide-ws` declares `tokio-tungstenite` or `rustls`, and only `polyoxide-core` declares `reqwest`.
- **Removals and allowlists.** Given `scripts/api_removals.py check` and the allowlist test, when the bundle is done, then both are green.
- **Live suites.** Given the clob `live_ws`, sports `live_api`, Binance `live_ws` and rtds `live_api` suites run with `-- --ignored`, then each passes or fails tagged `environmental` or `transient`, and the result is recorded.
- **Core untouched.** Given `git diff e114454 -- polyoxide-core`, when the bundle is done, then it is empty.

## Implementation Notes

Commits, in order: M-1 818dfc5, M-2 ff2d13c, M-3 86e81bd, M-4 ff36343, M-5 6d89684, M-6 06e4a40, M-7 513d01a, M-8 ae35629, M-9 f328582, M-10 bc68bc2. Each message names its behaviour changes and their tests. No review has run yet.

**Where the code departs from the plan, and why:**
- **M-1.** The scripted server cannot close after N received frames, so the two tests that ended on `recording_server`'s close use `Script::hold().drop_after(500 ms)`, and wait for the non-`PING` frames before asserting. Names and assertions are unchanged.
- **M-2.** The crypto test moved to the kit in M-2, as the task says, so clob's copy of the install ran untested until M-4 deleted it.
- **M-3.**
  - clob's normal `polyoxide-ws` dependency and the regenerated regions (crate list, publish order, nightly rows) came in M-3, not M-4: the move needs the kit, and `gen_registry.py --check` fails on any stale region. clob now publishes after `polyoxide-ws` (and Binance).
  - `live_ws`' three error builds changed in M-3, since `From<tungstenite::Error>` went there. Its R9 connect and the removal of its rustls install waited for M-4.
  - clob's own `ensure_crypto_provider` call stayed in `connect_ws` until M-4, though `connect_over` installs the provider too, so that M-4 is the deletion.
  - The new tests sit in a new module, `ws::client::connection_tests`, so the existing modules keep their counts. Besides the four the task names, it pins D12, `close()` after a server close, and the end after the first error.
  - `with_timeout` only ever fails with `Kind::ConnectTimeout`, so the explicit mapping back to `ConnectTimeout { url, timeout }` has an unreachable `Connection(err)` arm.
- **M-5.**
  - Both sports tiers read in `Mode::All`, the bare tier too, so the shared `classify` sees the same frames from both. The bare tier keeps its own debug "the sports feed closed" log.
  - The `Kind` mapping is two crate-private constructors, `SportsError::connect` and `::transport`, rather than a public `From<WsError>`, since two variants wrap `WsError`. The live suite maps `Kind::ConnectTimeout` itself.
  - sports' R11 test is inline in `supervised.rs` and its D19 test there too, so the protected `tests/supervision.rs` keeps 15. rtds' R11 test went into `tests/supervision.rs`, as L2's R1 to R3 tests did.
- **M-6.** Each override answers `ws_retry_after` on its `WsError` arm and falls back to `self.class().retry_after()` elsewhere, the trait's default.
- **M-7.**
  - The kit applies the hook after a failed attempt and before the first attempt of any outage begun by an error: a `start` in an outage, and a connection that ended on `End::Closed` or `End::Failed`. The latter two never carry a refusal today, so the hook answers `None` there.
  - The kit's reconnect WARN gains `?delay`. Every reconnecting venue's `backoff` setter documents R11.
  - The R11 tests use real time: a 0.4 s `Retry-After` (fractional seconds parse) over a 10 to 20 ms step, asserting the gap between accepted connections.
- **M-8.**
  - The router keeps the cause in one write-once `Arc<OnceLock<StopCause>>`, which the stream's `close()` and every `MembershipHandle` read.
  - The cause is the last item the path's stream yielded, when that was an `Err`. The kit yields the stop error last, so this is exact, except after a panic in the kit's task that followed a skipped `Frame` error.
  - A membership call that races the stop can still get `Stopped { cause: None }`.
  - Binance's scripted adapter gains `refuse_upgrade` (with headers) and `accepted_at`, for the R11 and M-D8 tests.
- **M-9.**
  - The owner fence also fails when an owner does not declare its crate as a normal dependency, so it cannot pass vacuously.
  - The `WsError` fence sweeps types of any visibility, strips comments (doc comments included) before matching, and requires the macro expanded as code. `polyoxide-ws` is not swept.
- **M-10.**
  - Of :34-39, the handshake half of the 451 row is done. Its `RtdsError::Server { status_code: 451 }` half is not, since no decision names it. It is re-filed as a narrowed entry.
  - CLAUDE.md :471 changed in M-1, and each other line in the commit that changed its rule (AD-21), so M-10 itself touches only `deferred-work.md`.
- **`docs/s1-removals.md` is unchanged.** The gate reports no new removal: the removed `From` impl and the changed field types report nothing, and `enum_marked_non_exhaustive` and `enum_unit_variant_changed_kind` (Binance's `Stopped`) are not removal lints.
- **MSRV was not checked locally** (rustc 1.95.0); CI's msrv job is the check.

**Test custody (AD-12).** Counts at e114454 and after, by `cargo test -p <crate> --all-features -- --list`:

| Suite | Before | After |
|---|---|---|
| clob `ws/client.rs` | 20 (connect 4, crypto 1, tests 6, dispatch 4, membership 5) | 27: crypto 0, moved to the kit by name; `connection_tests` +8 (R3, `close()` after a server close, the end after an error, D12, the per-step timeout, 451, 429, the DNS miss) |
| clob `ws/error.rs` | 2 | 5 (+451, +429, +503) |
| clob `auth.rs`, `market.rs`, `subscription.rs`, `user.rs` | 8, 20, 21, 21 | 8, 20, 21, 21 |
| clob `tests/live_ws.rs` | 2 | 2 |
| sports `tests/supervision.rs`, `bare.rs`, `failure_tags.rs` | 15, 6, 7 | 15, 6, 7 (M-D10 text only) |
| sports `supervised.rs` | 6 | 12 (+4 moved from `error.rs` by name, +D19, +R11) |
| sports `client.rs` | 7 | 7 |
| sports `error.rs` | 10 | 10 (−4 moved; +451, +429, +503, +the `Kind` mapping) |
| Binance `tests/supervision.rs`, `supervision_edges.rs` | 21, 7 | 21, 7 (M-D10 patterns only) |
| Binance `tests/supervision_kit.rs` | 14 (the spec's 10 predates L2's review) | 16 (+R11, +M-D8) |
| Binance `usdm/ws/protocol.rs` | 6 | 6 |
| Binance `usdm/ws/error.rs` | 5 | 7 (+503, +the stop's cause) |
| Binance `tests/failure_tags.rs` | 17 | 18 (+`streams::stopped_by_handshake_451`) |
| perps `ws/builder.rs`, `ws/error.rs` | 32, 4 | 33 (+R11), 5 (+`Retry-After`) |
| rtds `error.rs`, `tests/supervision.rs` | 16, 8 (the spec's 7 predates L2's review) | 17 (+`Retry-After`), 9 (+R11) |
| `polyoxide-venue/tests/ws_classification.rs` | 6 | 8 (+any status's `Retry-After`, +only a refusal carries one) |
| `polyoxide-ws` `backoff`, `connect`, `supervisor`, `tls` | 7, 8, 24, 1 | 10, 11, 28, 2 (+R11 floor ×3; +`connect_over` ×3; +R11 ×3 and the default hook; +clob's moved test) |
| CLI `tests/ws_binance.rs`, `ws_sports.rs` | 13, 16 | 13, 16 |
| `.github/scripts` | 927 | 940 (+2 M-8's row and twin check, +3 owner fence, +8 macro fence) |

Every test name at e114454 is present after, in its crate, except `a_default_crypto_provider_is_available_to_connect`, which is in `polyoxide-ws` (`tls::tests`), checked by name. Five tests were shown to fail without their fix: the clob R3 test with the kit's close-reply flush removed, the kit's five R11 tests with the floor removed, and the M-D8 test with the router's cause dropped.

**Gates, at bc68bc2:**
- Pass:
  - `cargo fmt --check`;
  - `cargo clippy --all-targets --all-features -D warnings`;
  - `cargo test --all-features --workspace` (109 result lines, all ok);
  - `RUSTDOCFLAGS=-D warnings cargo doc --workspace`;
  - `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`;
  - the six `--no-default-features --test headers` runs.
- Also pass:
  - `cargo publish --workspace --dry-run --no-verify` and `publish_order.py check-manifests`;
  - `uv run pytest tests/` in `.github/scripts` (940, the allowlists unchanged);
  - `gen_registry.py --check` and `live_unwraps.py`.
- `scripts/api_removals.py check --baseline v0.38.1` (cargo-semver-checks 0.51.0, rustc 1.95.0) is green: 46 removals, 46 listed, and every listed path compiles. `target/semver-checks` was deleted afterwards.
- `git diff e114454 -- polyoxide-core` is empty.

**The live suites, 2026-10-12:**
- clob `cargo test -p polyoxide-clob --features ws --test live_ws -- --ignored` (08:21:48–08:21:52 UTC): both tests failed tagged `auth-gated`, which is neither `environmental` nor `transient`. Both derive credentials from `POLYMARKET_PRIVATE_KEY`, which this machine does not set, and the keychain path needs `--features keychain` and a stored key, which was not tried. So the R9 probe has not run against the host; it opens its socket through the same `polyoxide_ws::connect` Binance's live suite passed with.
- sports `cargo test -p polyoxide-sports --test live_api -- --ignored` (08:21:52–08:24:53): 5 of 5 passed, the drift detector included.
- Binance `cargo test -p polyoxide-binance --features ws --test live_ws -- --ignored` (08:24:53–08:25:18): 4 of 4 passed.
- rtds `cargo test -p polyoxide-rtds --test live_api -- --ignored` (08:25:18–08:26:11): 3 of 3 passed.

**Incomplete or risky:**
- clob's `live_ws` needs a run with credentials before the AC on live suites is met.
- `RtdsError::Server { status_code: 451 }` stays a fault (re-filed); perps' `Stopped` has Binance's former gap (filed).
- A membership call racing a Binance stop may read `Stopped { cause: None }`.
- The R11 and M-1 tests depend on real time: R11's assert a lower bound of 0.4 s and a generous upper one, and M-1's assume a queued frame goes out within 500 ms.
- `divergences.md` (D20, R11, D19's and D10's wording, the R1, R3 and R9 cites) is the lead's and is unchanged.

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: clean
- `cargo test -p polyoxide-clob --no-default-features --test headers` -- expected: pass
- `(cd .github/scripts && uv run pytest tests/ -q)`, `python3 scripts/gen_registry.py --check`, `python3 scripts/live_unwraps.py` -- expected: pass
- `python3 scripts/api_removals.py check` -- expected: green (delete `target/semver-checks` afterwards)
- the four live suites with `-- --ignored` -- expected: pass, or a tagged environmental or transient failure, recorded
