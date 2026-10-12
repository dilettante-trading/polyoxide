# Epic 4 Context: One socket kit and supervisor, and the S1 release

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Every socket feed reconnects, stays alive and reports outages the same way, credential-free feeds stay light, and stage S1 ships as one release with a complete migration list for prader-rs. Epics 1–3 are merged to `main` (cb476e2) and unreleased; `main` reads 0.38.2, tagged before that merge. Stories 4.1–4.6 have landed on `aidanb/restructure-run`, ahead of `main` and at review (bundles K and L1, 96f96ec). They added `polyoxide-ws` (the socket kit, with the generic `Supervisor`), `polyoxide_venue::impl_ws_classification!` and the shared scripted server, and perps now runs on the `Supervisor`. Next is bundle L2: Story 4.7 moves Binance onto the `Supervisor`, and Story 4.8 puts rtds and sports on the kit's blocks. Then 4.9 does the same for clob's sockets, 4.10 reshapes every socket error onto the one table and leaves `polyoxide-ws` as the only crate that depends on tokio-tungstenite and rustls, and 4.11 cuts S1.

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

- **Behaviour is preserved unless a DRIFT row says otherwise.** Each DRIFT change is its own commit, named for its row, and gets an entry in `deferred-work.md` for 4.11's release notes, as L1's did.
  - **R1.** A handshake refused with 408, 425, 429 or 5xx is retried on every venue. rtds stops treating a 503 or a 429 at reconnect as fatal (4.8). Binance and sports already retry that set, and perps gained it in 4.6. The rule comes from `retrying_can_fix(err, |s| class_for_handshake_status(s).is_retriable())`.
  - **R2.** Every connect has a timeout, 10 s by default. rtds has none today (4.8). Binance and sports already have one.
  - **R3.** The RFC 6455 close reply reaches a venue only when that venue reads through the kit's `Connection`: Binance in 4.7, rtds in 4.8 and clob in 4.9. Sports already sends it.
  - **R9.** Three live tests install TLS themselves instead of calling `polyoxide_ws::ensure_crypto_provider`: Binance `live_ws` (4.7), sports `live_api` (4.8) and clob `live_ws` (4.9).
  - **R6 waits for S2.** rtds' `test-fixtures` feature keeps its name.
- **Each venue keeps its divergence rows**, at its `Protocol` or its call site. Only identical or parametric copies merge.
  - **D4, pings.** rtds pings on a quiet tick, Binance on the wall clock and clob every 10 s, and sports sends none. Never copy rtds' pump shape to a host that requires pings.
  - **D5 and D6, liveness and delivery.** For staleness, sports counts pings, rtds counts only decoded events, and Binance counts any frame, pongs and the server's pings included. Whether a connection delivered follows D5 and reaches `Backoff::after_connection_ended` as a `bool`.
  - **D7, shape.** sports stays a `Stream` state machine with no task, and rtds a `run(handler)` future. Neither goes on the `Supervisor`.
  - **D8, outage markers.** prader folds Binance's `Disconnected { path }` → `Reconnected { path }` pairs. rtds signals a reconnect with a new `Snapshot`.
  - **D9, Binance's limits.** Rotate at 23 h 50 min. A replay sends at most 200 names per request, one request per 200 ms, and enforces the 1024-stream cap first.
  - **D11, rtds.** Its snapshot and backfill semantics stay, as does its correction of the mislabelled spot snapshot.
  - **D13.** Core's jittered HTTP backoff is never unified with the socket `Backoff`.
  - **D18.** rtds and sports build nothing heavy.
  - **D19, use after close.** After `AlreadyClosed`, rtds stops while Binance and sports reconnect. This lives in each venue's recovery, not in `retrying_can_fix`.
  - **Sports and `Capacity`.** sports retries a connect that failed with `Capacity`, but `retrying_can_fix` gives up on it. Story 4.8 either keeps sports' rule at its call site or names the change as a DRIFT row.
- **The credential-free fence (AD-2, NFR2).** A socket-only crate may build `polyoxide-ws`'s dependencies, `polyoxide-venue`'s (none in venue's default build) and its own.
  - **Deny-list.** Never `reqwest`, `alloy*`, `polyoxide-core`, `governor`, `hmac`, `sha2` or `keyring`.
  - **Story 4.8** adds `ci/dep-allowlists/` files for `polyoxide-rtds` and `polyoxide-sports`, for each feature set. Regenerate them with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts`, and say why in the commit.
  - **The kit's tree.** Since 4.5, `polyoxide-ws`'s normal tree includes tokio's `macros`, `rt` and `sync`.
- **Test custody (AD-12, NFR7).**
  - **Protected suites:**
    - Binance: `tests/supervision.rs` (21), `supervision_edges.rs` (7), the 3 inline tests in `usdm/ws/supervised.rs`, and the usdm ws client's inline tests;
    - rtds: `tests/supervision.rs` (4) plus 15 inline, 19 in all;
    - sports: `supervision.rs` (15), `bare.rs` (6) and 6 inline, 27 in all;
    - also Story 2.2's class tables, the `tests/failure_tags.rs` twins (Binance, sports and rtds), the ws wire agreement and the live drift detectors.
  - **How tests move.** A moved test keeps its name and assertions. Each PR reports per-suite counts before and after, against its base commit, and shows that each moved supervision test also passed on the old loop. An old loop is deleted only after its suites pass against the `Supervisor`.
  - **Tests the kit already carries by name.** rtds' seven `backoff_schedule` tests are the kit's canonical `Backoff` suite. The venue servers' self-tests (Binance 2, rtds 8) are ported too. When a venue's copy goes, its count report shows these moved, not lost. Binance's and sports' own backoff tests have other names, and each must survive by name.
- **Removals (A4-1).**
  - **Order of changes.** Each of 4.7–4.9 first switches its venue's suites onto `polyoxide_ws::test_server` under the old loop. It then moves the loop, or adopts the blocks. Last, it removes the venue's `test_server` module.
  - **Listing.** Each removed path goes in `docs/s1-removals.md`, with no shims or re-exports, and the file stays cumulative against `v0.38.1`. A doc-hidden path's entry is marked `**Removed**` with its story and its replacement.
  - **Aliases.** cargo-semver-checks cannot see a type alias or a doc-hidden path, so both go in the file's own lists, as perps' `SupervisedPerpsWs` alias did.
  - **What 4.7 removes.** Binance's `supervised` module paths, its `MembershipHandle` and its `Recovery`.
  - **Fixtures stay with their venues, under their current features.** Binance's `usdm::ws::fixtures` and sports' `fixtures` sit behind `test-server`, and the CLI's tests import both. So unlike perps', those two `test-server` features stay. rtds' fixtures stay behind `test-fixtures`.
  - **Error enums** keep their names until S2. Changed variants are listed for prader.
- **Standing rules (AD-21).** CLAUDE.md changes in the commit that changes the rule.
  - **Sentences L2 makes false:**
    - the kit paragraph, which says perps runs on it and no other venue does;
    - the shared-server paragraph, which says Binance's, rtds' and sports' servers stay until 4.7 and 4.8;
    - the `ensure_crypto_provider` copies list;
    - Binance's supervised-tier paragraph;
    - rtds' and sports' "no other workspace crate".
  - **Generated text.** That last item, the crate list and the publish order are generated. Change `[package.metadata.polyoxide]`, then run `python3 scripts/gen_registry.py --write`. A crate that depends on `polyoxide-ws` publishes after it.
- **Gates (NFR9).** A red job withholds the release:
  - clippy and rustdoc with `-D warnings` (a `pub` doc must not link a `pub(crate)` item);
  - MSRV 1.91;
  - `cargo hack` on each feature;
  - the publish dry run;
  - the removals job;
  - the scripts job (allowlists, dependency fences, classify coverage, `gen_registry.py --check`).

## Technical Decisions

- **The `Supervisor` as built (AD-11, amended by A4-4).**
  - **Starting.** `Supervisor::new(protocol, timing)`, then one of:
    - `connect(membership)`, which returns the first attempt's failure;
    - `start(membership)`, which starts in an outage when `recover` would retry that failure.
  - **`Supervised<P>`** is the `Stream`. It has `close()`, and `membership()` gives a `Clone` `MembershipHandle<P>` whose `apply(Change)` waits for the answer.
  - **What a `Protocol` writes.**
    - `handshake`, built on every attempt.
    - `replay` of the membership, and `apply` of one change. `apply` uses the connection's request correlation, returns an `Ack` or an error, and decides how much of a partly refused change to record.
    - `record`, for a change made during an outage. `Offline::Recorded` answers at once and leaves the change to the replay. `Held`, the default, applies it after the reconnect.
    - `decode`, returning `Decoded`: nothing, one event, several events, or an error.
    - `is_live` and `delivered`, which see every frame.
    - `ping`, `wanted`, `queue` and `reset`.
    - `disconnected(Outage)` and `reconnected()`, the D8 markers.
    - `stalled` and `stopped`, which build the errors for a change left unanswered and for a stopped task.
  - **Recovery.** `is_retriable` is the one hook a venue must write. `recover` returns `Reconnect`, `Skip` (keep the connection and yield the error) or `Stop`. By default it reconnects exactly when `is_retriable` says so. A venue whose rule is not its class overrides `recover`, as perps does.
  - **Rules it enforces:**
    - each outage yields at most one `Disconnected`, and only for a declaring `Protocol`, and exactly one `Reconnected`, even when the membership empties mid-outage;
    - pings stay on the wall clock while a bounded queue is full: the task then reads nothing, and the wait does not count as silence;
    - no decoded event is dropped;
    - when the membership stops being wanted on a healthy connection, it closes politely with no marker;
    - connections rotate at `max_connection_age`, and the backoff resets after a connection that delivered;
    - each `replay` and `apply` is bounded by `stale_after`, since a request is not cancel-safe. One that outlives it is answered with `stalled`, and its connection is replaced;
    - the task ends promptly when the consumer drops the stream, even mid-change;
    - it logs under `polyoxide_ws`.
  - **perps is the worked example.**
    - Its public shape is a `pub type` alias over `Supervised<PerpsProtocol>`, plus a membership trait over `MembershipHandle`. The umbrella's `prelude` re-exports that trait.
    - Its socket error wraps `WsError`, and its `Classify` uses the macro.
- **Binance on the `Supervisor` (Story 4.7).**
  - **One `Supervisor` per routed path** (`/market`, `/public`), composed by a router.
    - An empty path is not wanted, and a router ends a path only by emptying its membership.
    - Use `record` for the changes Binance records during the backoff, and `start` to start a path in an outage.
    - Today's builder opens nothing for an empty start and opens a path on its first subscribe. Keep it that way: no test starts a `Supervisor` with an unwanted membership. The L1 review wrongly assumed that 4.7's builder refuses an empty start.
  - **Open: `NoAnswer` (deferred from L1).** Binance answers requests within 10 s and paces them 200 ms apart. W12 says to replace the connection after a `NoAnswer`. But on the `Supervisor` an error from `apply` goes to the caller, and the connection is kept unless the change outlived `stale_after`. Choose one:
    - `apply` ends the connection itself;
    - Binance uses the `stale_after` bound instead of an answer timeout;
    - the kit lets `apply` ask for a reconnect. That changes the `Supervisor`, so it needs a spine amendment.
  - **Risk (deferred from L1).** While a bounded queue is full the task answers no server pings. perps' host closed a client that withheld pongs, with `1011 pong_timeout` after about 70 s. Measure this, or have the task answer control frames while it waits, before Binance is put on the same rule.
  - **Binance's error rows.** If 4.7 wraps `WsError` in `UsdmWsError` as 4.6 did for perps, Binance's handshake 403 → `Restricted` (D14) stays Binance's own arm, ahead of the macro.
  - **What its suites may notice on the shared server.**
    - Its `ping_every` sends the first ping one period in, with a big-endian sequence payload. Binance's server pinged at once, with an empty payload.
    - A scripted close waits up to 1 s for the client's reply and counts it.
    - Binance's answer knobs (a held answer, ignoring later requests, refusals) are now closure state.
- **rtds and sports on the kit (Story 4.8).** Both depend on `polyoxide-ws` and `polyoxide-venue`, and still on no core, `reqwest` or `alloy`.
  - **rtds** keeps `run(handler)`. It adopts `Backoff`, connect with a timeout (R2), `retrying_can_fix` (R1), the kit's TLS, the close reply (R3) and the shared server, whose `after_first_frame` is rtds' record-before-sending.
  - **sports** keeps its state machine, pings as liveness and its rule for refusals that retrying cannot fix. It adopts the same blocks.
- **The socket error table (AD-15, amended by A4-2 and A4-3).**
  - **`WsError`** is opaque and read through `kind()`. No crate outside ws matches on tungstenite's error, and venues name tungstenite's types through `polyoxide_ws::tungstenite`.
  - **`impl_ws_classification!()`** expands to private `ws_class` and `ws_is_fault`, which a venue's `Classify` calls in its `WsError` arm. Only a 451 refusal is a non-fault.
  - **Class versus recovery.** The class is what consumers and nightly runs read. Recovery decides whether to reconnect, and the two must agree only on refused statuses.
- **Story 4.10.**
  - **Error enums.** It reshapes `UsdmWsError`, `SportsError`, `RtdsError` and clob's `WebSocketError` onto the macro, keeping Story 2.2's classes except R1's.
    - rtds' 451 becomes a non-fault.
    - Binance's `Stopped` keeps its stopping cause, so a stream stopped by a 451 tags `environmental`.
  - **Fences.** `test_dependency_fences.py` fails on a direct tokio-tungstenite or rustls dependency outside `polyoxide-ws`, and a new fence requires every crate that wraps `WsError` to expand the macro.
  - **Open.** Whether to surface a non-429 refusal's `Retry-After`.
- **clob (Story 4.9)** adopts the kit's TLS, `WsError`, `Connection` and server, which replaces its two loopback servers.
  - **What it keeps.** Binary decoded as text (D12), its 10 s `PING`, and `WebSocketWithPing`'s behaviour and shape.
  - **Connect.** Its per-address connect stays its own, wrapped by `with_timeout` and recorded as a new divergence row through `bmad-spec`.
  - **DNS.** Its "no addresses resolved" miss must reach the table as `Network`.
  - **Not supervised in S1.** Its supervised sockets come in Epic 7.
- **The S1 release (Story 4.11).**
  - **Version.** The bump raises the 0.x minor, since the semver job fails a patch bump on a major-level change. It is a separate commit on `main`, made after `git fetch` and a crates.io check, and it is the last commit before the tag. No bump is ever made on a loom branch.
  - **New crate names.** `polyoxide-venue`, `polyoxide-ws` and `polyoxide-cli` are published for the first time.
  - **Before the release.**
    - Confirm that the crates.io token covers `polyoxide*`, including publishing new crates, and that it expires after S3.
    - The `cargo` and `pypi` environments' restriction to `main` is the user's setting to change.
  - **Removal list.** `docs/s1-removals.md` is final and gives the call-site change beside each path.
  - **Release notes.** A consumer-impact section names R1–R4 and the log targets moved to `polyoxide_ws`, built from the 4.11 entries in `deferred-work.md`.
  - **After the release.** A smoke check proves `cargo install polyoxide-cli` works.
  - **Stage.** It changes in the root `Cargo.toml` (`[workspace.metadata.polyoxide] stage`) and in the spine, followed by `gen_registry.py --write`.

## Cross-Story Dependencies

- **Prerequisites, all landed.** Epics 1–3 are on `main`. Stories 4.1–4.6 are on `aidanb/restructure-run`, at review.
- **Within Epic 4:**
  - 4.7 needs the `Supervisor` (4.5), and 4.8 needs only the kit. Bundle L2 is 4.7 and 4.8 together, then come 4.9, 4.10 and 4.11.
  - 4.10 needs every migration (4.6–4.9) done, so that no venue still uses tungstenite directly.
  - 4.11 needs Epics 1–4 complete and CI green on `main`.
- **Later epics:**
  - Epic 5 (S2) moves the socket crates into `polyoxide-polymarket`, renames the error enums and does R6.
  - Epic 7's supervised clob sockets build on 4.5 and 4.9.
  - Epic 8's Kalshi socket uses the `Supervisor` with per-attempt handshake auth.
- **Unmerged amendments.** A2-1, A3-1 to A3-5 and A4-1 to A4-4 are accepted but not merged into the spine, and they override the spine and the epics file where those differ:
  - Story 4.4's suite switch moves to 4.6–4.9 (A4-1);
  - AD-15's rows named for tungstenite's variants become `Kind` (A4-2);
  - the injected rule applies to refused statuses only (A4-3);
  - AD-11 gains `apply`, `record`, `Skip` and a required `is_retriable` (A4-4).

  No story is assigned to the session that merges them.
