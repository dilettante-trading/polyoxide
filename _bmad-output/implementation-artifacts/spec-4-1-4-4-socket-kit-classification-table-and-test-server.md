---
title: 'Stories 4.1 to 4.4: the polyoxide-ws socket kit, one socket classification table, and one scripted test server'
type: 'feature'
created: '2026-10-10'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'cb476e2f52501e2fc94810626441428f33b5dcb9'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-4-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Five socket crates (clob, perps, Binance, rtds, sports) each carry their own copy of the same blocks:
- TLS install (W1), `Backoff` (W3), timing setters (W4) and deadline arithmetic (W5);
- the boxed tungstenite wrapper (W6), the handshake classifier (W7), connect timeout (W8) and close capture (W9);
- the bare-tier `poll_next` (W11), request/answer correlation (W12);
- a scripted test server and test helpers (W13, W15), about 1,450 lines.

The copies have drifted:
- perps and rtds have no connect timeout (R2);
- only sports sends the RFC 6455 close reply (R3);
- perps and rtds give up on handshake statuses their own `Classify` calls retriable (R1);
- five private `transport_class` copies hold the one socket table.

**Approach:** Add `polyoxide-ws`, which defines each block once, with no dependency on core or venue. Add `polyoxide_venue::impl_ws_classification!`, the one socket table, expanding against `::polyoxide_ws::WsError`. Add one scripted server under ws' `test-server` feature. No venue adopts the kit in this bundle; Stories 4.6–4.9 do.

**Decisions (made as the user's delegate, 2026-10-10):**
- **K-D1, scope.** Story 4.4's second AC (venue suites switch to the shared server, venue `test_server` modules removed and listed) moves to Stories 4.6–4.9, each switching the server under its old loop first, then the loop. That keeps one variable per change, and K removes no public API. Recorded as amendment A4-1.
- **K-D2, `WsError`.** An opaque `pub struct WsError(Box<Repr>)`, `Send + Sync + 'static`, implementing `std::error::Error` with the tungstenite error as `source()`. It is built by `From<tungstenite::Error>` and by kit constructors for its own failures (connect timeout, server close, no answer). `kind(&self) -> Kind<'_>` returns a `#[non_exhaustive]` enum with these variants:
  - `Io` (a TLS EOF included), `Protocol` and `ConnectionClosed`;
  - `Closed { code: Option<u16>, reason: &str }`, a server close frame;
  - `Refused { status: u16, retry_after: Option<&str> }`, a refused upgrade with its raw `Retry-After` header;
  - `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity` and `AlreadyClosed`;
  - `ConnectTimeout { after }` and `NoAnswer { after }`;
  - `Other` (WriteBufferFull, Utf8, any later tungstenite variant).

  No crate outside ws matches on tungstenite's error.
- **K-D3, the table.** `impl_ws_classification!()` takes no arguments and expands, in the invoking module, to two private functions, `fn ws_class(err: &::polyoxide_ws::WsError) -> Class` and `fn ws_is_fault(err: &::polyoxide_ws::WsError) -> bool`. A venue's `Classify` impl calls them on its `WsError`-carrying arm. It cannot implement the trait on `WsError` because of the orphan rule. Its rows:
  - **Network:** `Io`, `Protocol`, `ConnectionClosed`, `Other`, `ConnectTimeout` and `NoAnswer`.
  - **Close codes:** `Closed{code}` → `class_for_close_code(code)`, so 1000, 1001, 1006, 1011–1013 and no code are Network, and 1002, 1003, 1007–1010 and 4000–4999 are `VenueRefusal{code}`.
  - **InvalidRequest:** `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity` and `AlreadyClosed`.
  - **Refused upgrades:** `Refused{status, retry_after}` → `class_for_handshake_status(status).with_retry_after(parse_retry_after(v, Duration::MAX))`, so a 429 carries its wait, a 418 is `Restricted`, and a zero means no wait.
  - **`ws_is_fault`** is false only for `Refused{status: 451}`.

  It uses only `$crate::` paths for venue items.
- **K-D4, injected status rule (W7).** `polyoxide_ws::retrying_can_fix(err, retriable_status: impl Fn(u16) -> bool) -> bool` generalises sports' `retrying_can_fix`:
  - `Refused{status}` → the rule;
  - `Url`, `HttpFormat`, `Tls`, `AttackAttempt` and `Capacity` → false;
  - everything else → true.

  D19's `AlreadyClosed` split and a close code's meaning stay with each venue's recovery (Story 4.5's hook, or the rtds and sports call sites). Callers pass venue's rule, so the status set has one definition.
- **K-D5, re-export.** ws does `pub use tokio_tungstenite::tungstenite;`, the reqwest precedent, so later venue code names `Message` and `CloseFrame` through ws.

## Boundaries & Constraints

**Always:**
- **Dependencies.** `polyoxide-ws`'s normal dependencies are tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls (`ring`, `std`), plus serde and serde_json only if optional. All of them come from `[workspace.dependencies]`; add a `futures-util = "0.3"` entry there.
- **Logging.** It logs under the `polyoxide_ws` target and never installs a subscriber.
- **Tests.** A ported test keeps its name and assertions: rtds' seven `backoff_schedule` tests become ws' canonical `Backoff` suite, and rtds' copy stays until Story 4.8.
- **CLAUDE.md** (AD-21) is edited in the commit that changes the rule it states.
- **Generated regions** change only through `[package.metadata.polyoxide]` and `python3 scripts/gen_registry.py --write`.
- **Each block's doc comment** names its basis copy and the DFR rows that stay per venue (D4, D5, D6, D13, D19).
- **DRIFT R3 is its own commit**, named for the row, with the wire probe's result in its message and in Implementation Notes.

**Never:**
- **No venue changes.** No venue crate's `src` changes, no venue test moves, and no venue `test_server` is removed (K-D1).
- **No dependency between ws and venue or core**, in either direction, as a normal dependency. The only exception is the table test's path-only dev-dependency.
- No ws dependency on `polyoxide-test-support` (AD-1).
- **Two backoffs stay two.** Core's jittered `RetryConfig::backoff` is not unified with `Backoff` (D13).
- No Supervisor, `Protocol` or `Recovery` type; those are Story 4.5.
- No release, version bump or push.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected | Notes |
|---|---|---|---|
| Hung handshake | server accepts TCP, never upgrades | `connect` fails with `Kind::ConnectTimeout{after: 10s}` by default | W8, R2's block |
| 503 upgrade | refuse-status 503 | `Refused{503}`; `retrying_can_fix` true with venue's rule; `ws_class` Unavailable | R1's block |
| 429 with `Retry-After: 7` | refuse-status 429 + header | `ws_class` `RateLimited{Some(7s)}`; `Retry-After: 0` gives `None` | |
| 451 / 418 upgrade | refuse-status | both `Restricted`; `ws_is_fault` false for 451 only | |
| 404 upgrade | refuse-status 404 | `VenueRefusal`; `retrying_can_fix` false | |
| Server close 1011 / 1008 | close-code action | `Closed{Some(1011)}` Network / `Closed{Some(1008)}` `VenueRefusal{"1008"}` | W9 |
| Server close, bare tier | scripted close | bare tier flushes a close reply the server records, then ends; `closed()` is `Closed{code, reason}` | R3 |
| Ping during data-only read | server ping | not yielded in data mode; yielded in all-frames mode; pong still sent | W11 |
| Request with no answer | `answer` never replies | `Kind::NoAnswer{after}` at the answer timeout; pushes received meanwhile are yielded later in order | W12 |
| Backoff edges | ceiling < initial, zero initial, huge delay | clamp, grows, saturates (rtds' 7 tests) | W3 |
| Deadlines | no ping, no age | wait is stale only; all `None` → `Duration::MAX`; past deadline → zero | W5 |

</frozen-after-approval>

## Code Map

- **W1:** five byte-identical copies, each a `Once` plus `ring::default_provider().install_default()`:
  - rtds `polyoxide-rtds/src/client.rs:32`, with its test at `:216` (the basis);
  - clob `ws/client.rs:47`, perps `ws/mod.rs:41`, Binance `usdm/ws/mod.rs:92`, sports `client.rs:37`.
- **W3:** four identical copies, with defaults 500 ms and 60 s:
  - rtds `supervisor.rs:38` (the basis); its seven tests are in `mod backoff_schedule` (`:359-470`);
  - perps `ws/supervised.rs:40`, Binance `usdm/ws/supervised.rs:49`, sports `supervised.rs:54`.
- **W4:** the shared setter names are `ping_interval(Duration)`, `stale_after(Duration)`, `backoff(initial, max)`, `connect_timeout(Duration)` and `max_connection_age(Duration)`:
  - rtds `supervisor.rs:114-146`, perps `ws/supervised.rs:104-134`, Binance `usdm/ws/supervised.rs:175-221`, sports `supervised.rs:135-166`;
  - per-venue defaults are in epic-4-context D4.
- **W5:** Binance `usdm/ws/supervised.rs:904` takes the min of the ping, stale and age waits, saturating (the basis). perps `:435` takes two of them. Do not copy rtds `supervisor.rs:288`.
- **W6:** perps `ws/error.rs:42/102`, Binance `usdm/ws/error.rs:29/110`, rtds `error.rs:17/96`, clob `ws/error.rs:10/49`. sports boxes at its call sites.
- **W7:** sports' `retrying_can_fix` (`error.rs:85`) is the basis for K-D4. The table to replace is the private `transport_class` copies: sports `error.rs:108`, clob `ws/error.rs:58`, perps `ws/error.rs:162`, rtds `error.rs:176`, and Binance `usdm/ws/error.rs:159`, which adds 403 → `Restricted` (D14, which stays Binance's).
- **W8:** sports `client.rs:47` and Binance `client.rs:153`. clob's per-address connect (`client.rs:63-110`) stays clob's own.
- **W9:** Binance `record_close` and `closed_error` (`usdm/ws/client.rs:279/286`), and sports `closed()` (`client.rs:97`).
- **W11:** sports `client.rs:167-200` (the basis): it flushes the close reply at `:195`, and its frame classifier is at `:76`. perps (`client.rs:244`) and Binance (`client.rs:384`) drain a buffered queue first. The socket type alias is at sports `client.rs:26`.
- **W12:** Binance `request` and `await_answer` (`usdm/ws/client.rs:292-360`) are the basis; they have the 10 s answer timeout and optional pacing. perps' `take_id` and `await_response` (`ws/client.rs:202-240`) must also be expressible; it has no timeout today.
- **W13 and W15:**
  - the servers are sports `src/test_server.rs` (271 lines), perps `ws/test_server.rs` (354), Binance `usdm/ws/test_server.rs` (364) and rtds `src/test_server.rs` (470). Their shared skeleton is `start(Vec<Script>)`, where the last script repeats, plus `connection_count`, `wait_for` and the `select!` interval idiom;
  - the actions are the union of their scripts: reject, stall, refuse-upgrade status (with headers), send or push, `push_every`, `ping_every` (be-bytes sequence), `close_after` with an optional code, `drop_after`, `go_silent` and `hold`;
  - the recorders are per-connection frames, paths, closes, close replies, pongs, protocol pings received, accepted-at times and ended counts;
  - the answer hooks are perps' `answer` (`:156`) and Binance's (`:174`, with per-connection state). rtds records the first frame before sending (`:207`, a load-bearing comment);
  - the helpers are `fast` and `next_event` (perps `supervised.rs:591-610`, Binance `tests/supervision.rs:29-55`) and sports' `next_item` and `drain`.
- **`polyoxide-venue`:**
  - `class_for_status` (`status.rs:23`), and `class_for_handshake_status` and `class_for_close_code` (`socket.rs:29-46`);
  - `Class::with_retry_after` (`class.rs`), and `parse_retry_after(&str, Duration)` (`retry_after.rs:27`);
  - exports are in `lib.rs:53-57`.
- **Registration examples:**
  - `polyoxide-venue/Cargo.toml` is the manifest to model. The crate-metadata keys are at `scripts/gen_registry.py:102`, and a crate with no venue, mirrors or live suite writes only README `readme-crates` and CLAUDE.md `claude-crate-count`, `claude-graph` and `claude-publish-order`;
  - the root `Cargo.toml` has `members` at `:2-17` and `[workspace.dependencies]` at `:30-56`. tokio-tungstenite is 0.26 with `rustls-tls-native-roots`, and rustls is 0.23 with `ring` and `std`.
- **CI scripts:**
  - `.github/scripts/tests/test_dependency_fences.py` is the pattern for a new check: a constant, a `*_problems(metadata)` function, a live test with a non-vacuity assertion, and a synthetic test;
  - `test_classify_coverage.py:39` has the `EXEMPT` set, which holds only venue's `ClassifiedError` today;
  - `ci.yml`'s scripts job runs pytest, and cargo is available there.
- **`docs/s1-removals.md`:** unchanged; K adds a crate and removes nothing.

## Tasks & Acceptance

**Execution:** each item below is one commit, in order. Commits name their story; run `cargo fmt`, `clippy` and the touched tests before each.
- [x] **K1 (4.1).** Covers `polyoxide-ws/` (new: `Cargo.toml`, `src/lib.rs`, `src/tls.rs`, `src/backoff.rs`, `src/timing.rs`, `src/error.rs`), the root `Cargo.toml`, `Cargo.lock`, the generated regions, `ci/dep-allowlists/` and `.github/scripts/tests/test_dep_allowlists.py`.
  - **Crate and registration.** Create the crate with `[package.metadata.polyoxide]` `readme` and `notes`, register it and run `gen_registry.py --write`.
  - **W1.** `ensure_crypto_provider` is public; port rtds' test.
  - **W3.** `Backoff` with the seven tests.
  - **W4.** `Timing` (pub fields or getters; `Default` uses ping `Some(20s)`, stale 30 s, backoff 500 ms/60 s, connect 10 s and age `None`), plus a `timing_setters!` macro. A builder lists the setters it wants (`ping_interval`, `stale_after`, `backoff`, `connect_timeout`, `max_connection_age`); each keeps its doc and writes `self.timing`, and a test builder invokes all five.
  - **W5.** A deadline function: saturating, with `Duration::MAX` meaning never.
  - **W6.** `WsError` and `Kind` (K-D2), and the `tungstenite` re-export (K-D5).
  - **Allowlist check.** `ci/dep-allowlists/polyoxide-ws.txt` and `polyoxide-ws@test-server.txt` (the latter lands with K5) hold the sorted unique crate names from `cargo tree -p polyoxide-ws -e normal --prefix none [--features …]`. The test diffs each file against cargo, prints the extra and missing names, and asserts non-vacuity. It also carries a synthetic parser test.
- [x] **K2 (4.2).** Covers `polyoxide-ws/src/connect.rs`, `src/bare.rs` and `src/request.rs`.
  - **W8.** `connect(request, timeout)` and `DEFAULT_CONNECT_TIMEOUT` (10 s). It calls `ensure_crypto_provider` and returns the `Socket` alias. A `with_timeout(timeout, fut)` wraps a caller's own connect future, for clob.
  - **W7.** `retrying_can_fix` (K-D4).
  - **W9.** The close frame is captured into `Kind::Closed`.
  - **W11.** The bare-tier `Stream` has a data-only mode (skips Ping, Pong and Frame) and an all-frames mode. It drains a buffered queue first and exposes `closed()` and `close()`. The close reply is not sent yet.
  - **W12.** Request/answer correlation: it allocates ids, buffers pushes, and takes an optional minimum request interval and an answer timeout giving `NoAnswer`. It is generic over the envelope, through a caller-supplied builder and an answer matcher. Tests model both the perps envelope and the Binance envelope against a minimal in-test server; K5's server replaces that server.
- [x] **K3 (DRIFT R3).** Covers `polyoxide-ws/src/bare.rs`, `examples/close_reply_probe.rs` and Implementation Notes.
  - **The probe.** Before the fix, it connects with tokio-tungstenite over a stream wrapper that counts bytes written after a Close frame is read. It uses today's bare pattern (return on Close, no flush) and runs once against perps' `WS_URL` (`wss://ws.perpetuals.polymarket.com/v1/ws`, `polyoxide-perps/src/ws/mod.rs:30`), idle until the host's 60 s close.
  - **Recording the result.** Record whether the host sent a Close frame and the byte counts in Implementation Notes and the commit message. If no live host sends a Close within 3 minutes, record that and rely on the code evidence (only sports flushes).
  - **The fix.** On a server Close, the bare tier flushes the reply before ending, and `close()` sends a close and flushes.
- [x] **K4 (4.3).** Covers `polyoxide-venue/src/socket.rs` or a new `src/ws_table.rs`, `polyoxide-venue/Cargo.toml` (path-only dev-dependency on `polyoxide-ws`), `polyoxide-venue/tests/ws_classification.rs` and `test_classify_coverage.py`.
  - **The macro.** `impl_ws_classification!` (K-D3) is `#[macro_export]`ed with its docs.
  - **The table test.** The test invokes the macro and covers every row: each `Kind`, close codes 1000–1013 and 4000/4999, statuses 100–599 against `class_for_handshake_status`, the 429 `Retry-After` and its zero, and the 451 non-fault. It also asserts that `retrying_can_fix(e, venue_rule)` equals `ws_class(e).is_retriable()` for every `Refused` status.
  - **The exemption.** Add `("polyoxide-ws", "WsError")` to `EXEMPT`, with the reason that it is classified through each venue's enum.
- [x] **K5 (4.4).** Covers `polyoxide-ws/src/test_server/` (feature `test-server`, doc-hidden like the venue copies, built only from crates ws already depends on) and its allowlist file.
  - **The server.** `ScriptedServer::start(Vec<Script>)` provides the actions, recorders, `connection_count`, `wait_for` (5 s) and the interval idiom from the Code Map. It takes a pluggable `answer` with per-connection state, plus an option to record the first frame before sending (rtds).
  - **The helpers (W15).** `next_event`, `fast() -> Timing` (ping 50 ms, stale 300 ms, backoff 10/20 ms) and ping counting.
  - **Self-tests.** Port the venue servers' self-tests (perps 5, Binance 2, rtds 8), expressing perps' and Binance's answers as in-test closures to prove the hook fits. Move K2's correlation tests onto this server.
- [x] **K6 (docs).** Covers `CLAUDE.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-4.md` (new) and `deferred-work.md`.
  - **CLAUDE.md.** The Module Organization WebSocket paragraph names `polyoxide-ws` and what each venue adopts in Stories 4.6–4.9. The `test-server` text names the shared server. The scripts-job sentence names the allowlist check. Fix the stale "socket errors keep their inherent `is_retriable`/`retry_after` until Epic 4" clause, since none has one now.
  - **Amendments.** A4-1 is K-D1, A4-2 is K-D2/K-D3 (macro shape, `Kind`) and A4-3 is K-D4.
  - **Deferred work.** Release-notes entries addressed to Story 4.11.

**Acceptance Criteria:**
- **ws's dependencies.** Given the workspace, when `cargo tree -p polyoxide-ws -e normal` gains any crate outside its allowlist, then the CI scripts job fails, naming it; `cargo tree -i polyoxide-venue -p polyoxide-ws -e normal` and the same for core find nothing.
- **Every feature builds.** Given `cargo hack check --each-feature --no-dev-deps --ignore-private`, when it runs, then `polyoxide-ws` passes with and without `test-server`.
- **Coverage, packaging and registration.** Given CI's scripts job, when it runs, then classify coverage passes with ws' exemption, the package dry run includes `polyoxide-ws`, and `gen_registry.py --check` is clean.
- **The macro and the status set.** Given a crate invoking `impl_ws_classification!()`, when it builds, then it needs no tungstenite dependency of its own, and the table test proves the status set has one definition.
- **The full suite.** Given the full suite (fmt, clippy `-D warnings`, `cargo test --all-features --workspace`, rustdoc `-D warnings`, scripts pytest, gates), when K6 lands, then every check passes and no venue test count changes.

## Implementation Notes

**DRIFT R3's wire probe (K3), run 2026-10-10 15:29 UTC against `wss://ws.perpetuals.polymarket.com/v1/ws`.** `cargo run -p polyoxide-ws --example close_reply_probe [-- --flush]`, the two patterns run side by side:
- **The host does not idle-close a client that reads.** A first run with an ordinary stream saw a protocol `Ping` every 10 s for 180 s and no close: tungstenite's automatic pongs count as inbound messages. The probe therefore withholds every byte the client writes after the handshake until the Close is read (168 bytes of pongs), so the host sees a silent client.
- **It then sent a Close frame:** after 70.0 s, code 1011 (`Error`), reason `pong_timeout`.
- **Today's bare pattern** (return on the Close, no flush) wrote **0 bytes** to the TCP socket in the 2 s after reading it: no close reply.
- **One flush after the Close** wrote **42 bytes**: one TLS record carrying the reply, which echoes the code and reason (2 + 4 + 14 bytes framed, plus 22 of TLS 1.3 record overhead).
- **The fix** flushes the queued reply before the bare tier yields its last item, in `poll_next` and in a request's wait, and `close()` flushes it on a connection the server already closed. Three tests hold it, and all three fail with the flush removed.
- **For Story 4.6:** perps' idle rule as observed is a 10 s server ping and a 1011 `pong_timeout` after about 70 s without a pong, not "60 s without an inbound message"; a client that reads answers the pings itself. `docs/specs/perps/OBSERVED.md` does not record this yet.

**Where the code departs from the plan, and why:**
- **The classify-coverage exemption landed in K1, not K4.** `WsError` is swept as soon as it exists, so leaving `("polyoxide-ws", "WsError")` for K4 would have left K1–K3's scripts suite red. CLAUDE.md states the exemption from K1, and K4 adds that the macro is how venues classify it.
- **The answer hook is `ScriptedServer::start_answering(scripts, answer)`.** `start(scripts)` answers nothing; `answer` is called once per connection with its index and returns that connection's answerer, `FnMut(&str) -> Vec<Message>`, which owns the protocol state: Binance's streams, a held answer, perps' dead pings and refusals. Binance's `hold_answer`, `ignore_later_requests` and `refuse`, and perps' `refuse`, `answer_pings`, `pong_status` and `answer_subscribes_after_first`, are closure state, not `Script` fields.
- **One action beyond the four servers' union: `Script::then_cut()`,** which drops the TCP connection after `send` with no close frame. K2's failed-read test needed it once the loopback helpers went. `Script::hold()` is the default (sports called it `silent()`); `after_first_frame()` is rtds' record-before-sending. `close_after` waits a second for the client's reply and counts it, as sports' did; Binance's did not wait. `ping_every` is sports' shape (first ping one period in, big-endian sequence payload); Binance's sent its first ping at once with an empty payload, which its suites, at 50 ms pings, should not notice.
- **The bare tier is `Connection`, the request API is on it.** `Connection::new(socket, Mode)`; `request(build, answer)` takes the id it allocated in both closures, `await_answer(answer)` serves a protocol ping's pong without taking an id, and `take_id`/`next_id` cover perps' and Binance's id handling. A frame the matcher calls `Push` is kept only when the connection's `Mode` yields it. `last_inbound()` counts every frame, those read while awaiting an answer included, for Binance's liveness rule (D5).
- **MSRV was not checked locally.** There is no rustup on this machine; nothing used is newer than 1.91 by inspection, and CI's msrv job is the check.
- **Venue test counts are unchanged by construction:** `git diff cb476e2 -- polyoxide-binance polyoxide-perps polyoxide-rtds polyoxide-sports polyoxide-clob` is empty.
- **Correction to the R3 note (review):** the three tests failed when `record_close` stopped marking the reply queued, which removes every flush. With only `close()`'s own `flush_reply` removed, `close_after_the_server_closed_succeeds` still passes, because `poll_next` has already flushed the reply before `close()` runs; that flush has no test of its own (deferred, review finding G3).

## Spec Change Log

## Review Triage Log

Three layers on `bundle-k-review.diff` (cb476e2..8631916; the spec, epic context and sprint status left out). E = edge-case, B = blind, G = verification-gap. 39 findings: 18 patched, 1 deferred, 20 rejected.

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| E1 | `request()` after a failed read takes an id and sends on the dead socket | low | `request.rs:98-107` checks `closed()` only; `fail()` leaves `close` `None` | patch (A) |
| E2 | `min_request_interval(Duration::MAX)` overflows `last + interval` | low | `request.rs:104` adds an `Instant` and a `Duration` unchecked; `checked_add` is a direct correction | patch |
| E3 | `take_id` + `send` skip the pacing | low | only `request()` sleeps and records `last_request` (`request.rs:101-108`); no caller yet, so a doc fix | patch (B) |
| E4 | the answer timeout does not bound the send | medium | `request.rs:107` awaits `send` outside `await_answer`'s timeout; a peer that stops reading hangs the call | patch |
| E5 | a read failure in `read_until` ends the stream with no cause | false | the `Err` goes to the request's caller, who owns the connection; `poll_next` does the same (`Err`, then `None`) | reject |
| E6 | an All-mode pong waits for the next read when the flush is `Pending` | low | needs write backpressure as a ping arrives; the fix adds state | reject |
| E7 | the close-reply flush has no deadline | low | needs a peer that sends Close and then stops reading; supervision staleness covers it; the fix adds a deadline | reject |
| E8 | `close()` after a failed read errors | low | `bare.rs:130-137` sends on the dead socket; same cause as E1 | patch (A) |
| E9 | zero timings are unguarded | low | perps' basis setters (`ws/supervised.rs:110-121`) take zero as well; only a caller's error reaches it; the fix adds guards | reject |
| E10 | a refused upgrade's `Retry-After` is lost for every status but 429 | low | frozen K-D3 carries the wait only through `RateLimited`, as epic-4-context planned; no socket reads `Retry-After` today, so nothing regresses; the fix edits the frozen spec (noted for Story 4.10) | reject |
| E11 | `Capacity` is `false` in `retrying_can_fix`, where sports retries it | low | frozen K-D4 lists `Capacity` as false; sports is unchanged until Story 4.8, which keeps or names it (noted) | reject |
| E12 | the test server's `close` reads one frame, so an in-flight frame hides the reply | medium | `test_server/mod.rs:418-426`; a pong or app ping sent before the reply leaves `close_reply_count` at 0, the assertion 4.6–4.9 rely on | patch (C) |
| E13 | `go_silent` parks before the loop, so `drop_after` never fires | low | `test_server/mod.rs:448-452` awaits `pending()` in `send_scripted`; `drop_after`'s doc says "whatever else is happening" | patch (D) |
| E14 | the server drops the socket on a client Close without sending its reply | low | `test_server/mod.rs:583-587` returns with the reply queued | patch (E) |
| E15 | zero or `MAX` `push_every`/`ping_every`/`drop_after` panic in the server task | low | test-only misuse; the fix adds guards | reject |
| E16 | an invalid `refuse_upgrade` status panics in the server task | low | test-only misuse; the fix adds validation | reject |
| E17 | a failing `cargo tree` hides its stderr | low | `test_dep_allowlists.py` runs with `check=True`, and `CalledProcessError` does not print stderr | patch |
| E18 | the probe drops TLS records, so its post-Close bytes prove only that the client wrote | low | `close_reply_probe.rs:77-80` withholds below TLS; the absence today (0 bytes) stands, the proof of the fix is a client write | patch (F) |
| E19 | the probe cannot parse an IPv6 literal host | low | the probe targets perps' `WS_URL` only | reject |
| E20 | `test-server` adds tokio-macros to the tree | false | tokio is already a dependency, and its `macros` feature brings in its own proc-macro crate, which the allowlist records; AD-1 forbids test-support, not a feature | reject |
| B1 | the macro's rationale wrongly blames the orphan rule for venue | medium | venue may implement its own trait for any type; what stops it is AD-2 (no ws dependency). Wrong in `ws_table.rs`, the coverage docstring, CLAUDE.md and A4-2 | patch |
| B2 | the macro's absolute `::polyoxide_ws` path breaks under a renamed dependency; Kind is not tied to a version; a new Kind is silently Network | low | no workspace crate renames it, and both crates release in lockstep at one workspace version; the wildcard is mandatory for a `#[non_exhaustive]` enum and documented | reject |
| B3 | = E10 | low | as E10 | reject |
| B4 | nothing forces a crate wrapping `WsError` to expand the macro | low | no crate wraps `WsError` before Story 4.10, so a fence would check nothing now (noted for Story 4.10) | reject |
| B5 | a request after a failed read reports `AlreadyClosed` | low | same cause as E1; after the patch it refuses before sending | patch (A) |
| B6 | = E3 | low | as E3 | patch (B) |
| B7 | = E12 | medium | as E12 | patch (C) |
| B8 | the allowlist docstring says "the tree a consumer builds", but `-e normal` leaves out build dependencies; rtds and sports are unpinned | low | ring's `cc` is a build dependency missing from the list, so fix the docstring; rtds and sports are Story 4.8's by plan | patch (docstring) |
| B9 | the ws README describes only Story 4.1 | medium | `polyoxide-ws/README.md` omits connect, the bare tier, correlation and `test-server`; it is the crates.io page on first publish | patch |
| B10 | `Timing`'s doc says its defaults are shared; `#[non_exhaustive]` blocks other crates from constructing it | low | rtds pings at 5 s and sports stales at 45 s with no ping, so fix the doc; the fields are `pub`, so a venue mutates `Timing::default()`, and the shape is false | patch (doc) |
| B11 | `Kind::Closed` also covers an end with no close frame, and its text merges no-code with abrupt | low | `record_close(None)` at `bare.rs:224` gives `Closed{None}`; both classes are Network, so a doc fix | patch |
| B12 | the probe's module doc still says perps idle-closes at 60 s; its byte count proves only a client write | low | the probe found a `1011 pong_timeout` at 70 s, with pongs withheld; second half as E18 | patch (F) |
| B13 | = E13 | low | as E13 | patch (D) |
| B14 | = E9 | low | as E9 | reject |
| B15 | = E7 | low | as E7 | reject |
| B16 | the server sends no reply to a client Close; `client_frames()` copies `received()`'s doc; `drain_reads_to_the_end` asserts nothing | low | (a) as E14; (b) and (c) confirmed in `test_server/` | patch (E, docs, test) |
| G1 | `last_inbound()` is unchecked across a request | gap | removing `touch()` at `request.rs:153` leaves every test passing | patch |
| G2 | an All-mode Close read during a request is unchecked | gap | removing `keep()` at `request.rs:156` leaves every test passing | patch |
| G3 | `close_after_the_server_closed_succeeds` cannot see `close()`'s own flush | gap | `poll_next` has already flushed it; reaching `close()` with the reply still queued needs a `Pending` write, which loopback does not give | defer |

## Design Notes

A venue's `Classify` impl after Story 4.10, using the generated functions:

```rust
polyoxide_venue::impl_ws_classification!();

impl Classify for PerpsWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => ws_class(err),
            Self::Stalled { .. } => Class::Network,
            // the venue's own arms
        }
    }
    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connection(err) if !ws_is_fault(err))
    }
}
```

Two kinds of classification, kept apart:
- **What the caller reads.** `ws_class`, in venue, is the one table consumers and nightly runs see.
- **What recovery decides.** `retrying_can_fix`, in ws, decides only whether reconnecting can help a failed connect. Its status rule is injected, so the two can never disagree on a status. The table test pins that.

## Verification

**Commands:**
- `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass; venue test counts unchanged
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: clean
- `(cd .github/scripts && uv run pytest tests/ -q)` and `python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py` -- expected: pass
- `cargo publish --workspace --dry-run --no-verify` -- expected: includes polyoxide-ws
