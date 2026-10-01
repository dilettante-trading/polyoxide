# Sports: the live match feed as its own crate

**Date:** 2026-10-01
**Branch:** `aidanb/sports-api`
**Slice:** the whole of `wss://sports-api.polymarket.com/ws`. Gamma's sports REST routes
(`/sports`, `/sports/market-types`, `/teams`, `/teams/{id}`) stay in `polyoxide-gamma` and
are not part of this spec.
**Plan:** `docs/superpowers/plans/2026-10-01-polyoxide-sports.md`, written next.

## Goal

Give polyoxide a `polyoxide-sports` crate that streams live match state from Polymarket's
sports socket with no credentials and no signing stack, survives a season of reconnects,
and tells its caller exactly when scores may be stale. Remove the sports channel from
`polyoxide-clob`, where reading a score currently costs the whole `alloy` build.

## Scope

In:

- A bare stream and a supervised stream (staleness on protocol pings, reconnect, outage
  markers), both yielding a `MatchUpdate` type pinned to captured frames.
- `GameKey`, one identifier across numeric and cricket game ids, with the gamma
  reconciliation query documented.
- Removal of the sports surface from `polyoxide-clob` in the same release (breaking).
- `polyoxide ws sports` in the CLI.
- `docs/specs/sports/`, holding the moved mirror and an `OBSERVED.md`.
- A fixture capture script and live tests that act as this host's only drift detector.

Out: Python bindings (Python has no WebSocket bindings for any feed yet), a per-game state
table, a shared WebSocket support crate for rtds, perps and sports, and any change to
gamma's sports routes.

## Decisions taken during brainstorming

| Question | Decision | Why |
|---|---|---|
| What "the sports API" means here | A dedicated crate for the socket | The host serves only `/health` and `/ws`; everything else sports-related is gamma |
| Own crate or fix inside clob | Own crate, kept thin | Credential-free feed on its own host, the same case that produced `polyoxide-rtds` |
| Fate of clob's sports channel | Removed in the same release | Two copies of one frame type would drift; clob's `Channel` never belonged to this host |
| What the crate yields | The frame, a `GameKey`, and outage markers | Everything else (dedupe, transitions, state tables) is a caller's `HashMap` and a policy the crate cannot know |
| Consumer | `polyoxide ws sports` | Gives the crate a real consumer and a live test through the CLI suite |
| Supervision shape | A `Stream` state machine with no task | Nothing is ever sent, so there is nothing to schedule; avoids the close-deadlock and quiet-tick-ping classes found in perps review |

Rejected: copying the perps task-and-channel supervisor (its command queue, shutdown path
and buffer serve outbound pings and membership, which sports has neither of), and
extracting a shared WebSocket crate first (rewrites two shipped, mutation-tested
supervisors whose differences are load-bearing). A task-based supervisor would expose the
same public type, so one can be introduced later without an API change.

## Venue contract

There is no usable published contract. `docs.polymarket.com/asyncapi-sports.json`
documents a `slug`-keyed payload and a text `"ping"`/`"pong"` every 5 seconds; neither
exists on the wire. The oracle is captured frames, and this host is excluded from
`nightly-schema.yml`.

| Fact | July 2026-07-25 (229 frames, 5 min) | October 2026-10-01 06:23 UTC (121 frames, 5 min) |
|---|---|---|
| Keep-alive | Protocol PING roughly every 15 s, no text ping | Protocol PING every 15.0 s from connect, no text ping |
| Subscription | None; every live match is pushed | Same |
| Required on every frame | `leagueAbbreviation`, `score`, `period`, `live`, `ended` | Same, 121/121 |
| Identifier | `gameId` (int), or `metadataGameId` (string) on cricket | Same; 91 and 30 frames |
| `eventState` | On soccer and tennis | On no frame, tennis included |
| Longest data gap | Not measured | 12.8 s |
| Frames identical to that game's previous frame | Not measured | 56 of 121 |

October findings that shape the design:

- **Rebroadcast.** Each live esports game is re-sent every 20 s whether or not it changed;
  tennis irregularly, 22 to 153 s apart. A frame is not a change.
- **The ended frame is sent once.** 14 of 15 finished games produced exactly one
  `ended: true` frame. A reconnect across that moment never learns the game ended.
- **Cricket sweep.** 14 cricket matches carried the same `finishedTimestamp` to the
  millisecond, most never seen live. Cricket's `ended` may mean "dropped from the feed".
- **League is free text.** `wta challenger` arrived with a space.
- **Status casing** is not normalised: `InProgress`, `inprogress`, `running`, `finished`.
- **Reconciliation.** `GET gamma-api/events?game_id=<gameId>` returned exactly one event
  for each of four ids tried, including one ended game showing `ended: true` and its final
  score. `game_id=<metadataGameId>` is refused with `invalid integer`, so cricket games
  cannot be reconciled through gamma.

## Design

### Crate layout

```
polyoxide-sports/
  Cargo.toml       no in-workspace dependencies; feature test-server
  README.md        included as crate docs, so its examples are doctests
  src/
    lib.rs          re-exports, SPORTS_WS_URL
    update.rs       MatchUpdate, GameKey
    client.rs       SportsWs (bare tier), crypto-provider install, connect with timeout
    supervised.rs   SportsWsBuilder, SupervisedSportsWs, Event, Backoff
    error.rs        SportsError
    test_server.rs  scripted server; cfg(test) or feature test-server
  tests/
    fixtures/       captured frames plus PROVENANCE.md
    supervision.rs  required-features = ["test-server"]
    live_api.rs     #[ignore]
scripts/capture_sports_fixtures.py
```

Dependencies: `tokio` (`net`, `time`, `macros`, `rt`), `tokio-tungstenite`,
`futures-util`, `serde`, `serde_json`, `thiserror`, `tracing`, and `rustls` with `ring` and
`std` declared explicitly, for the same reason `polyoxide-rtds` declares them (see the
TLS note in CLAUDE.md). This is the fourth copy of `ensure_crypto_provider`.

### Types

`MatchUpdate` replaces clob's `SportsUpdateMessage`. Same fields and requiredness:

| Field | Wire key | Type |
|---|---|---|
| `league_abbreviation` | `leagueAbbreviation` | `String` |
| `score` | `score` | `String` |
| `period` | `period` | `String` |
| `live` | `live` | `bool` |
| `ended` | `ended` | `bool` |
| `game_id` | `gameId` | `Option<u64>` |
| `metadata_game_id` | `metadataGameId` | `Option<String>` |
| `home_team`, `away_team` | `homeTeam`, `awayTeam` | `Option<String>` |
| `status` | `status` | `Option<String>` |
| `elapsed` | `elapsed` | `Option<String>` |
| `finished_timestamp` | `finishedTimestamp` | `Option<String>` |
| `event_state` | `eventState` | `Option<serde_json::Value>` |
| `extra` | every other key | `serde_json::Map`, flattened |

Every `Option` field carries `skip_serializing_if = "Option::is_none"` so serialisation is
lossless. The struct derives `Debug, Clone, PartialEq, Serialize, Deserialize` and is
`#[non_exhaustive]`. Its docs say plainly that consecutive frames for a game are often
identical and that `PartialEq` is the intended way to drop them.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameKey {
    /// `gameId`. Gamma's events list accepts it: `gamma.events().list().game_id([id as i64])`.
    Game(u64),
    /// Cricket's `metadataGameId`. No gamma filter accepts it.
    Metadata(String),
}

impl MatchUpdate {
    /// `None` only for a frame carrying neither identifier, which no capture has shown.
    pub fn key(&self) -> Option<GameKey>;
}

impl Display for GameKey { /* the id as sent: decimal for Game, verbatim for Metadata */ }
```

The key is not enforced at decode time, so a frame with a future identifier still reaches
the caller.

```rust
#[non_exhaustive]
pub enum Event {
    /// Boxed: `MatchUpdate` is 296 bytes against 40 for the other variants, which
    /// trips clippy's `large_enum_variant`, and the repo allows no lint exceptions.
    Update(Box<MatchUpdate>),
    /// The connection was lost. Scores are stale from here until `Reconnected`.
    Disconnected { reason: SportsError },
    /// A new connection is up. Games that ended during the gap were not re-sent;
    /// reconcile them through gamma by `GameKey::Game`.
    Reconnected,
}
```

### Errors

`SportsError`, `#[non_exhaustive]`, via `thiserror`:

| Variant | When |
|---|---|
| `Connect { source }` | The handshake failed |
| `ConnectTimeout { after }` | The handshake did not finish in time |
| `Closed { code, reason }` | A close frame arrived, or the stream ended (`code: None`) |
| `Transport { source }` | A read failed mid-connection |
| `Stale { after }` | Nothing inbound, pings included, for `stale_after` |
| `Decode { raw, source }` | A text frame did not parse as `MatchUpdate` |

On the supervised tier only `Decode` reaches the caller as `Err`. Every other variant
arrives inside `Disconnected`, because the stream has already acted on it.

### Bare tier

`SportsWs::connect()` and `SportsWs::connect_to(url)` connect with a 10 s timeout and
implement `Stream<Item = Result<MatchUpdate, SportsError>>`.

- A protocol `Ping` or `Pong` is consumed and the loop reads again at once. Tungstenite
  0.26.2 queues the pong when the ping is read and flushes it at the top of the next
  `read()` (`protocol/mod.rs`, `read` and the `OpCtl::Ping` arm), so the pong leaves as
  long as the caller is polling.
- A text frame that fails to decode yields `Err(Decode)` and the stream continues.
- Binary frames are logged at `debug` and skipped; none has been observed.
- A close ends the stream with `None`. A transport error yields `Err(Transport)`, then
  `None`. The bare tier never reconnects.

### Supervised tier

```rust
let feed = SportsWsBuilder::new()       // url, stale_after, backoff, connect_timeout
    .connect()                          // eager; Err if the first connect fails
    .await?;                            // SupervisedSportsWs: Stream<Item = Result<Event, SportsError>>
```

`poll_next` drives a three-state machine. There is no spawned task.

```
Reading(socket, deadline)
   frame          -> yield Update; deadline = now + stale_after
   ping or pong   -> deadline = now + stale_after; read again
   bad frame      -> yield Err(Decode); read again
   close, error,
   or deadline    -> yield Disconnected { reason }; go to Backoff
Backoff(sleep)    -> elapsed: go to Connecting
Connecting(fut)   -> ok:  yield Reconnected; go to Reading
                  -> err: log at WARN; go to Backoff with a longer delay
```

| Setting | Default | Reason |
|---|---|---|
| `stale_after` | 45 s | Three missed 15 s server pings; data cannot set this because a quiet hour has none |
| Backoff | 500 ms doubling to 60 s | As perps. Resets only after a connection that delivered at least one inbound message |
| Connect timeout | 10 s | As clob's sports channel today |

One `Disconnected` and one `Reconnected` per outage, however many attempts it takes. The
stream never ends while held. Dropping it drops the socket; there is no `close()`.

Known limit: pongs leave only while the caller polls. A caller that blocks for longer than
the server's pong deadline, which is unmeasured, will be dropped and see `Disconnected`
then `Reconnected`. That is a visible failure, never silent loss.

### Removal from clob

One commit marked `!`, with the migration in its body. It deletes `ws/sports.rs`,
`WS_SPORTS_URL`, `WebSocket::connect_sports`, `Channel::Sports`, `ChannelType::Sports`,
the re-exports of `SportsMessage` and `SportsUpdateMessage`, the unit tests
`every_real_sports_frame_reaches_the_caller` and `sports_url_uses_its_own_host`, the
sports section of `polyoxide-clob/README.md`, and the ws module's doc bullet. The two
sports tests in `polyoxide-clob/tests/live_ws.rs` move to `polyoxide-sports/tests/live_api.rs`.
`Channel` is already `#[non_exhaustive]`, so only code naming the variant breaks. The
release is 0.36.0.

### CLI

```
polyoxide ws sports [--league atp,wta] [--game 1712005,id2704098174740616]
                    [--changes-only] [--format pretty|json] [-n COUNT] [-t DURATION]
```

- Reads `SupervisedSportsWs`.
- `--league` and `--game` are `Vec<String>` with `value_delimiter = ','`. League matching
  ignores case. `--game` matches `GameKey`'s `Display`, so it accepts either id kind.
  Frames with no key never match a `--game` filter.
- `--changes-only` keeps the last update per `GameKey` and drops a frame equal to it.
  Keyless frames always pass.
- Updates go to stdout: one line per update in pretty mode (league, id, teams, score,
  period, live or ended), compact `MatchUpdate` JSON in json mode. `Disconnected` and
  `Reconnected` go to stderr as one line each.
- `-n` counts printed updates. `-t` exits 0 when it elapses.
- Filtering and output live in `run_with(events, stdout, stderr)`, generic over any
  `Stream<Item = Result<Event, SportsError>>`, on the pattern of `DataCommand::run_with`.
- `polyoxide-cli` gains a `polyoxide-sports` dependency and keeps `polyoxide-clob/ws` for
  `ws market` and `ws user`.

### Spec docs

- `git mv docs/specs/clob/asyncapi-sports.json docs/specs/sports/asyncapi.json`. The one
  annotation naming `SportsUpdateMessage` is updated to `MatchUpdate`; upstream's text is
  untouched.
- New `docs/specs/sports/INDEX.md` (host, routes `/ws` and `/health`, crate) and
  `docs/specs/sports/OBSERVED.md` (the July and October findings above).
- Live references updated: CLAUDE.md, `SELF-HEALING.md`, `docs/specs/gamma/OBSERVED.md`,
  the exclusion comment in `.github/workflows/nightly-schema.yml`, `docs/specs/INDEX.md`
  (row moves to the implemented table), and `docs/specs/clob/websocket.md` (sports rows
  removed, pointer to `sports/`). Dated plans and specs are records and stay as written.

### Workspace wiring

| Place | Change |
|---|---|
| `Cargo.toml` | Member, and `polyoxide-sports` in `[workspace.dependencies]` at the shared version |
| `polyoxide/Cargo.toml` | `sports = ["dep:polyoxide-sports"]`, in `full`, not in default |
| `polyoxide/src/lib.rs` | `pub use polyoxide_sports`; prelude exports `SportsWs`, `SportsWsBuilder`, `SupervisedSportsWs`, `MatchUpdate`, `GameKey`, `Event as SportsEvent`. No `PolymarketError` variant, matching rtds |
| `release.yml` | `CRATES` order `core, rtds, sports, perps, relay, gamma, data, clob, polyoxide`, and its comment |
| `nightly-behavioral.yml` | `{ crate: polyoxide-sports, suite: live, timeout: 15, flags: "--test live_api" }` |
| CLAUDE.md | Dependency graph, publishing order, the WebSocket section's sports paragraphs, the CLI paragraph |
| Root `README.md` | Crate table row |

## Testing

Every test names the bug it exists to catch, and the plan breaks the code on purpose to
show each supervision test going red.

### Fixtures

`scripts/capture_sports_fixtures.py OUT_DIR [SECONDS]` records the socket, keeps the first
frame per distinct top-level key-set and per `eventState.type`, and writes `PROVENANCE.md`
(date, duration, frames per league, ping times). The initial set is the five July frames
copied verbatim from clob (the only `eventState` examples) plus the October shapes:
`wta challenger`, `status: finished`, and a `gameId` frame with `finishedTimestamp`. Unit
tests load the files with `include_str!`.

### Frame type

- Every fixture parses.
- Lossless round trip: `to_value(from_str(f)) == from_str::<Value>(f)` for every fixture.
- Each field reads its own key: changing one key in a fixture changes exactly that field.
- `GameKey` for each kind, for neither, and its `Display`.
- A frame missing any one required field is rejected.

### Supervision (`tests/supervision.rs`, scripted server, stale limits in hundreds of ms)

| Test | Bug it catches |
|---|---|
| Server sends only protocol pings for three stale periods; no `Disconnected` | Staleness counting only data, which would drop every quiet hour |
| Server goes silent; `Disconnected { Stale }` then `Reconnected` | Staleness not enforced |
| Server pings; a pong with the same payload arrives within 1 s | The read loop stopping after a ping |
| Server closes after two frames; updates, `Disconnected`, `Reconnected`, updates | Missing reconnect or misordered markers |
| Server refuses N connects then accepts; exactly one `Disconnected`, one `Reconnected` | A marker per attempt instead of per outage |
| Server accepts and drops at once; delays keep doubling | Backoff resetting on a connection that delivered nothing |
| Bad frame; `Err(Decode)` then the next update on the same connection | One bad frame tearing down the socket |
| Stream dropped; server sees the socket close | A leaked connection |
| First connect refused; `connect()` returns `Err` | Silent retry when the setup is wrong |

`Backoff` also gets unit tests for doubling, the cap and the reset rule. The bare tier
gets the ping, decode and close cases.

### CLI

- Parse tests pass `--league atp,wta` and `--game 1,id2` and assert what the filter reads.
- `run_with` over a `futures::stream::iter` of events: league filter, both id kinds,
  `--changes-only` (drops a repeat, keeps a change, keeps keyless frames), `-n`, markers
  only on stderr, every stdout line in json mode parses.
- Live: `ws sports -n 1 --format json` prints one parseable line.

### Live (`#[ignore]`, nightly)

- Bare: a frame arrives and parses (moved from clob).
- Bare: the connection survives 40 s (moved from clob).
- Supervised: held 50 s, past the 45 s stale limit, with no `Disconnected`.
- Raw socket: at least two protocol pings in 40 s, none more than 20 s apart. The 50 s test
  alone cannot show that pings count, because data resets staleness whenever a match is
  live; this one shows the signal the 45 s default rests on is there.
- Wire agreement: every frame in a window round-trips losslessly and leaves `extra`
  empty. A new top-level key fails the test and names it. `nightly-schema.yml` excludes
  this host, so this test is its drift detector, and the tracking issue it raises is the
  intended outcome.

A timeout waiting for frames says the run can "legitimately time out", which
`classify_failures.py` already treats as environmental.

### Gate

`cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo nextest run`, doctests, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
--all-features --workspace`. The last matters because the README is compiled and a
private intra-doc link would withhold the release tag.

## Open questions settled by evidence, not by this spec

- **North American leagues are unseen.** Neither capture overlapped NFL, NBA, MLB or NHL
  play, nor a soccer weekend. Their frames may carry new keys or `eventState` types. The
  live wire-agreement test names any new top-level key, but only for leagues live while
  it runs, and the 06:00 UTC nightly misses these. A capture during a busy window (a
  Saturday or Sunday afternoon UTC) is how they will be seen, and should refresh the
  fixtures before release if one can be scheduled.
- **The server's pong deadline** is unmeasured. The upstream page's 10 s describes a text
  exchange the server does not use.
- **Cricket reconciliation.** Whether any gamma field carries `metadataGameId` is
  unknown; this spec documents that no filter accepts it.
