---
title: 'Stories 4.1 to 4.4: the polyoxide-ws socket kit, one socket classification table, and one scripted test server'
type: 'feature'
created: '2026-10-10'
status: 'in-progress'
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
- [ ] **K1 (4.1).** Covers `polyoxide-ws/` (new: `Cargo.toml`, `src/lib.rs`, `src/tls.rs`, `src/backoff.rs`, `src/timing.rs`, `src/error.rs`), the root `Cargo.toml`, `Cargo.lock`, the generated regions, `ci/dep-allowlists/` and `.github/scripts/tests/test_dep_allowlists.py`.
  - **Crate and registration.** Create the crate with `[package.metadata.polyoxide]` `readme` and `notes`, register it and run `gen_registry.py --write`.
  - **W1.** `ensure_crypto_provider` is public; port rtds' test.
  - **W3.** `Backoff` with the seven tests.
  - **W4.** `Timing` (pub fields or getters; `Default` uses ping `Some(20s)`, stale 30 s, backoff 500 ms/60 s, connect 10 s and age `None`), plus a `timing_setters!` macro. A builder lists the setters it wants (`ping_interval`, `stale_after`, `backoff`, `connect_timeout`, `max_connection_age`); each keeps its doc and writes `self.timing`, and a test builder invokes all five.
  - **W5.** A deadline function: saturating, with `Duration::MAX` meaning never.
  - **W6.** `WsError` and `Kind` (K-D2), and the `tungstenite` re-export (K-D5).
  - **Allowlist check.** `ci/dep-allowlists/polyoxide-ws.txt` and `polyoxide-ws@test-server.txt` (the latter lands with K5) hold the sorted unique crate names from `cargo tree -p polyoxide-ws -e normal --prefix none [--features …]`. The test diffs each file against cargo, prints the extra and missing names, and asserts non-vacuity. It also carries a synthetic parser test.
- [ ] **K2 (4.2).** Covers `polyoxide-ws/src/connect.rs`, `src/bare.rs` and `src/request.rs`.
  - **W8.** `connect(request, timeout)` and `DEFAULT_CONNECT_TIMEOUT` (10 s). It calls `ensure_crypto_provider` and returns the `Socket` alias. A `with_timeout(timeout, fut)` wraps a caller's own connect future, for clob.
  - **W7.** `retrying_can_fix` (K-D4).
  - **W9.** The close frame is captured into `Kind::Closed`.
  - **W11.** The bare-tier `Stream` has a data-only mode (skips Ping, Pong and Frame) and an all-frames mode. It drains a buffered queue first and exposes `closed()` and `close()`. The close reply is not sent yet.
  - **W12.** Request/answer correlation: it allocates ids, buffers pushes, and takes an optional minimum request interval and an answer timeout giving `NoAnswer`. It is generic over the envelope, through a caller-supplied builder and an answer matcher. Tests model both the perps envelope and the Binance envelope against a minimal in-test server; K5's server replaces that server.
- [ ] **K3 (DRIFT R3).** Covers `polyoxide-ws/src/bare.rs`, `examples/close_reply_probe.rs` and Implementation Notes.
  - **The probe.** Before the fix, it connects with tokio-tungstenite over a stream wrapper that counts bytes written after a Close frame is read. It uses today's bare pattern (return on Close, no flush) and runs once against perps' `WS_URL` (`wss://ws.perpetuals.polymarket.com/v1/ws`, `polyoxide-perps/src/ws/mod.rs:30`), idle until the host's 60 s close.
  - **Recording the result.** Record whether the host sent a Close frame and the byte counts in Implementation Notes and the commit message. If no live host sends a Close within 3 minutes, record that and rely on the code evidence (only sports flushes).
  - **The fix.** On a server Close, the bare tier flushes the reply before ending, and `close()` sends a close and flushes.
- [ ] **K4 (4.3).** Covers `polyoxide-venue/src/socket.rs` or a new `src/ws_table.rs`, `polyoxide-venue/Cargo.toml` (path-only dev-dependency on `polyoxide-ws`), `polyoxide-venue/tests/ws_classification.rs` and `test_classify_coverage.py`.
  - **The macro.** `impl_ws_classification!` (K-D3) is `#[macro_export]`ed with its docs.
  - **The table test.** The test invokes the macro and covers every row: each `Kind`, close codes 1000–1013 and 4000/4999, statuses 100–599 against `class_for_handshake_status`, the 429 `Retry-After` and its zero, and the 451 non-fault. It also asserts that `retrying_can_fix(e, venue_rule)` equals `ws_class(e).is_retriable()` for every `Refused` status.
  - **The exemption.** Add `("polyoxide-ws", "WsError")` to `EXEMPT`, with the reason that it is classified through each venue's enum.
- [ ] **K5 (4.4).** Covers `polyoxide-ws/src/test_server/` (feature `test-server`, doc-hidden like the venue copies, built only from crates ws already depends on) and its allowlist file.
  - **The server.** `ScriptedServer::start(Vec<Script>)` provides the actions, recorders, `connection_count`, `wait_for` (5 s) and the interval idiom from the Code Map. It takes a pluggable `answer` with per-connection state, plus an option to record the first frame before sending (rtds).
  - **The helpers (W15).** `next_event`, `fast() -> Timing` (ping 50 ms, stale 300 ms, backoff 10/20 ms) and ping counting.
  - **Self-tests.** Port the venue servers' self-tests (perps 5, Binance 2, rtds 8), expressing perps' and Binance's answers as in-test closures to prove the hook fits. Move K2's correlation tests onto this server.
- [ ] **K6 (docs).** Covers `CLAUDE.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-4.md` (new) and `deferred-work.md`.
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

## Spec Change Log

## Review Triage Log

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
