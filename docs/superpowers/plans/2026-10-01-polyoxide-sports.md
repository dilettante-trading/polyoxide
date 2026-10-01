# polyoxide-sports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Polymarket's live sports feed (`wss://sports-api.polymarket.com/ws`) out of `polyoxide-clob` into a dependency-free `polyoxide-sports` crate with a bare stream, a task-free supervised stream, a `polyoxide ws sports` CLI command, and live tests that act as the host's drift detector.

**Architecture:** One crate with no in-workspace dependencies. `client.rs` opens a socket and classifies each inbound message; both tiers share that classifier. `supervised.rs` is a three-state `Stream` machine (reading, waiting, connecting) with no spawned task, because nothing is ever sent to this server. Protocol pings count as liveness. Clob loses its sports channel in the same release.

**Tech Stack:** Rust 1.91, `tokio` 1.41, `tokio-tungstenite` 0.26 (tungstenite 0.26.2), `futures-util` 0.3, `serde`/`serde_json`, `thiserror` 2, `tracing`, `rustls` 0.23 (`ring`, `std`), `clap` 4.5 in the CLI.

**Spec:** `docs/superpowers/specs/2026-10-01-polyoxide-sports-design.md`. Read it before Task 1.

---

## Conventions for every task

- Work in this worktree on branch `aidanb/sports-api`. Never switch branches, rebase, or create worktrees.
- Every commit message ends with these two lines, after a blank line:

  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
  ```

- Conventional commit subjects. git-cliff builds the changelog from them, and a `!` marks a breaking change.
- Never create scratch or backup files inside the repository. Loom stages new files automatically, and stray ones leave ghost index entries. Use `$TMPDIR` or the session scratchpad.
- If `rustc` dies with signal 15 or exit 254, that is earlyoom on this machine, not a code fault. Rerun with `-j 2`.
- Doc comments may intra-link only to items that already exist when the task lands. `cargo doc` runs with `-D warnings` in CI, and a link to a missing item fails it. Use plain backticks for anything later tasks create.
- Fixture frames are copied byte for byte from captures. Never hand-write or edit one.
- "Prove it" steps break the code on purpose to show a test catches a specific bug. Always revert the break before committing, and confirm with `git diff` that only intended changes remain.

## File map

| Path | Responsibility |
|---|---|
| `polyoxide-sports/Cargo.toml` | Manifest; `test-server` feature; `[[test]]` gates |
| `polyoxide-sports/README.md` | Crate README, compiled as doctests |
| `polyoxide-sports/src/lib.rs` | Crate docs and re-exports |
| `polyoxide-sports/src/update.rs` | `MatchUpdate`, `GameKey` |
| `polyoxide-sports/src/error.rs` | `SportsError` |
| `polyoxide-sports/src/client.rs` | `SPORTS_WS_URL`, `open`, `classify`, `SportsWs` (bare tier) |
| `polyoxide-sports/src/supervised.rs` | `Backoff`, `SportsWsBuilder`, `SupervisedSportsWs`, `Event` |
| `polyoxide-sports/src/fixtures.rs` | Captured frames as constants (`cfg(test)` or `test-server`) |
| `polyoxide-sports/src/test_server.rs` | Scripted local WebSocket server (`cfg(test)` or `test-server`) |
| `polyoxide-sports/tests/fixtures/*.json`, `PROVENANCE.md` | Captured frames and where they came from |
| `polyoxide-sports/tests/bare.rs` | Bare tier against the scripted server |
| `polyoxide-sports/tests/supervision.rs` | Supervised tier against the scripted server |
| `polyoxide-sports/tests/live_api.rs` | `#[ignore]` live tests, including wire agreement |
| `scripts/capture_sports_fixtures.py` | Fixture capture from the live socket |
| `polyoxide-cli/src/commands/ws/sports.rs` | `polyoxide ws sports`: args, filter, `run_with` |
| `polyoxide-cli/tests/ws_sports.rs` | `run_with` over scripted event lists |
| `polyoxide/{Cargo.toml,src/lib.rs,README.md}` | `sports` feature and prelude |
| `polyoxide-clob/src/ws/*`, `README.md`, `tests/live_ws.rs` | Sports channel removed |
| `docs/specs/sports/{INDEX.md,OBSERVED.md,asyncapi.json}` | Spec docs; mirror moved from `docs/specs/clob/` |
| `.github/workflows/{release.yml,nightly-behavioral.yml,nightly-schema.yml}` | Publish order, nightly row, exclusion comment |
| `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/**` | Documentation references |

---

### Task 1: Scaffold the crate with its captured fixtures

**Files:**
- Create: `polyoxide-sports/Cargo.toml`, `polyoxide-sports/src/lib.rs`, `polyoxide-sports/src/fixtures.rs`
- Create: `polyoxide-sports/tests/fixtures/{soccer,tennis_event_state,esports,cricket,cricket_finished,league_with_space,finished_numeric}.json`, `polyoxide-sports/tests/fixtures/PROVENANCE.md`
- Modify: `Cargo.toml` (workspace members and `[workspace.dependencies]`)

- [ ] **Step 1: Write the manifest**

`polyoxide-sports/Cargo.toml`:

```toml
[package]
name = "polyoxide-sports"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Rust client for Polymarket's live sports score WebSocket"
keywords = ["polymarket", "websocket", "sports", "scores"]
categories = ["api-bindings", "web-programming::websocket"]

[features]
# Exposes the scripted local server and the captured frames, so integration
# tests here and in polyoxide-cli can use them. Not for consumers; both
# modules are `#[doc(hidden)]`.
test-server = []

# Tasks 6 and 8 add `[[test]]` entries here as they create those files.
# Cargo refuses a manifest whose named test file does not exist yet.

[dependencies]
tokio = { workspace = true, features = ["macros", "net", "time", "rt"] }
tokio-tungstenite = { workspace = true }
futures-util = "0.3"
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
# Needed only to install a process-default rustls CryptoProvider. See
# `ensure_crypto_provider` in src/client.rs. `std` is declared explicitly
# because this crate has no neighbour that turns it on transitively.
rustls = { version = "0.23", default-features = false, features = ["ring", "std"] }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time", "net"] }
```

- [ ] **Step 2: Register the crate in the workspace**

In the root `Cargo.toml`, add `"polyoxide-sports",` to `members` directly after `"polyoxide-rtds",`, and add this line directly after the `polyoxide-rtds = ...` line in `[workspace.dependencies]`:

```toml
polyoxide-sports = { path = "polyoxide-sports", version = "0.35.0" }
```

- [ ] **Step 3: Write the fixture files, byte for byte, with no trailing newline**

Run from the repository root:

```bash
mkdir -p polyoxide-sports/tests/fixtures && cd polyoxide-sports/tests/fixtures
printf '%s' '{"gameId":90106111,"leagueAbbreviation":"kor","homeTeam":"Gimcheon Sangmu FC","awayTeam":"Daejeon Hana Citizen FC","status":"InProgress","eventState":{"type":"soccer","createdAt":"2026-07-25T12:02:18.395759286Z","updatedAt":"2026-07-25T12:02:18.395759286Z","score":"2-1","elapsed":"74","period":"2H","live":true,"ended":false},"score":"2-1","elapsed":"74","period":"2H","live":true,"ended":false}' > soccer.json
printf '%s' '{"gameId":5968822,"leagueAbbreviation":"atp","homeTeam":"Alexander Bublik","awayTeam":"Quentin Halys","status":"inprogress","eventState":{"type":"tennis","createdAt":"2026-07-25T12:02:23.120698555Z","updatedAt":"2026-07-25T12:02:23.120698555Z","score":"4-6, 1-2","period":"S2","live":true,"ended":false,"tournamentName":"Generali Open","tennisRound":"Final"},"score":"4-6, 1-2","period":"S2","live":true,"ended":false}' > tennis_event_state.json
printf '%s' '{"gameId":1590176,"leagueAbbreviation":"lol","homeTeam":"Caldya Esport","awayTeam":"Galions Sharks","status":"running","score":"000-000|0-0|Bo1","period":"1/1","live":true,"ended":false}' > esports.json
printf '%s' '{"metadataGameId":"id2703680373085574","leagueAbbreviation":"cricket","score":"21-178","period":"Live","live":true,"ended":false}' > cricket.json
printf '%s' '{"metadataGameId":"id2703438269077680","leagueAbbreviation":"cricket","score":"116-38","period":"FT","live":false,"ended":true,"finishedTimestamp":"2026-07-25T12:06:54.595448902Z"}' > cricket_finished.json
printf '%s' '{"gameId":6361496,"leagueAbbreviation":"wta challenger","homeTeam":"Hiroko Kuwata","awayTeam":"Wushuang Zheng","status":"inprogress","score":"0-0","period":"S2","live":true,"ended":false}' > league_with_space.json
printf '%s' '{"gameId":6352662,"leagueAbbreviation":"wta","homeTeam":"Yulia Starodubtseva","awayTeam":"Alina Charaeva","status":"finished","score":"7-5, 5-7, 2-6","period":"FT","live":false,"ended":true,"finishedTimestamp":"2026-10-01T06:23:23.907040294Z"}' > finished_numeric.json
cd -
```

The first five are the July constants from `polyoxide-clob/src/ws/sports.rs`; confirm with `grep -c 90106111 polyoxide-clob/src/ws/sports.rs` (expect 2: the constant and one test assertion). The last two are from the 2026-10-01 capture.

- [ ] **Step 4: Write `polyoxide-sports/tests/fixtures/PROVENANCE.md`**

```markdown
# Sports fixture provenance

Every file here is a frame captured verbatim from
`wss://sports-api.polymarket.com/ws`. None is hand-written: a fabricated
sports fixture once carried an `event_type` field the server has never
sent, and the parser shipped filtering on it with passing tests.

Refresh with `scripts/capture_sports_fixtures.py` into a scratch
directory, copy the frames worth keeping here, and list each one in
`src/fixtures.rs`. `every_fixture_file_is_listed` fails otherwise.

| File | Captured | Shape it covers |
|---|---|---|
| `soccer.json` | 2026-07-25 | `eventState` of type `soccer`; `elapsed` present |
| `tennis_event_state.json` | 2026-07-25 | `eventState` of type `tennis`, adding `tournamentName` and `tennisRound` |
| `esports.json` | 2026-07-25 | The commonest shape: no `eventState`, no `elapsed` |
| `cricket.json` | 2026-07-25 | `metadataGameId` only: no `gameId`, `homeTeam`, `awayTeam` or `status` |
| `cricket_finished.json` | 2026-07-25 | A cricket frame carrying `finishedTimestamp` |
| `league_with_space.json` | 2026-10-01 | `leagueAbbreviation` of `wta challenger`, with a space |
| `finished_numeric.json` | 2026-10-01 | `gameId` with `finishedTimestamp` and `status: finished` |

## Captures

- **2026-07-25**, 229 frames over five minutes, covering soccer, tennis,
  cricket and the lol, val, cs2, dota2 and mlbb esports titles. These five
  frames were first held as constants in `polyoxide-clob/src/ws/sports.rs`.
- **2026-10-01, 06:20 UTC**, 121 frames over five minutes across atp, wta,
  wta challenger, cricket, mlbb and dota2. No frame carried `eventState`.
  Protocol pings arrived every 15.0 s, and no text ping was sent.
```

- [ ] **Step 5: Write `polyoxide-sports/src/fixtures.rs`, tests included**

```rust
//! Frames captured verbatim from `wss://sports-api.polymarket.com/ws`, for
//! the tests in this crate and in `polyoxide-cli`.
//!
//! Provenance is in `tests/fixtures/PROVENANCE.md`. Nothing here may be
//! hand-written; refresh with `scripts/capture_sports_fixtures.py`.

/// Soccer, with an `eventState` block and `elapsed` (2026-07-25).
pub const SOCCER: &str = include_str!("../tests/fixtures/soccer.json");
/// Tennis, whose `eventState` adds `tournamentName` and `tennisRound` (2026-07-25).
pub const TENNIS_EVENT_STATE: &str = include_str!("../tests/fixtures/tennis_event_state.json");
/// Esports, the commonest shape: no `eventState`, no `elapsed` (2026-07-25).
pub const ESPORTS: &str = include_str!("../tests/fixtures/esports.json");
/// Cricket, identified by `metadataGameId` alone (2026-07-25).
pub const CRICKET: &str = include_str!("../tests/fixtures/cricket.json");
/// A finished cricket match (2026-07-25).
pub const CRICKET_FINISHED: &str = include_str!("../tests/fixtures/cricket_finished.json");
/// A league label containing a space (2026-10-01).
pub const LEAGUE_WITH_SPACE: &str = include_str!("../tests/fixtures/league_with_space.json");
/// A finished match with a numeric `gameId` (2026-10-01).
pub const FINISHED_NUMERIC: &str = include_str!("../tests/fixtures/finished_numeric.json");

/// Every fixture, keyed by its file stem.
pub const ALL: [(&str, &str); 7] = [
    ("soccer", SOCCER),
    ("tennis_event_state", TENNIS_EVENT_STATE),
    ("esports", ESPORTS),
    ("cricket", CRICKET),
    ("cricket_finished", CRICKET_FINISHED),
    ("league_with_space", LEAGUE_WITH_SPACE),
    ("finished_numeric", FINISHED_NUMERIC),
];

#[cfg(test)]
mod tests {
    use super::ALL;

    #[test]
    fn every_fixture_file_is_listed() {
        // A refreshed capture copied into the directory but not listed here
        // would otherwise never be tested.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut listed: Vec<String> = ALL.iter().map(|(name, _)| (*name).to_owned()).collect();
        listed.sort();
        assert_eq!(on_disk, listed, "tests/fixtures and ALL disagree");
    }

    #[test]
    fn every_fixture_is_one_captured_line() {
        for (name, frame) in ALL {
            assert!(!frame.contains('\n'), "{name} is not a single captured line");
            let value: serde_json::Value =
                serde_json::from_str(frame).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(value.is_object(), "{name} is not a JSON object");
        }
    }

    #[test]
    fn no_fixture_carries_the_invented_event_type_field() {
        for (name, frame) in ALL {
            assert!(!frame.contains("event_type"), "{name} carries event_type");
        }
    }
}
```

- [ ] **Step 6: Write `polyoxide-sports/src/lib.rs`**

```rust
//! Rust client for Polymarket's live sports feed at
//! `wss://sports-api.polymarket.com/ws`.

#![warn(missing_docs)]

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p polyoxide-sports`
Expected: `3 passed` in the `fixtures::tests` module.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock polyoxide-sports
git commit -F - <<'MSG'
feat(sports): scaffold polyoxide-sports with captured fixtures

Seven frames captured from sports-api.polymarket.com, five from the
2026-07-25 capture held in polyoxide-clob and two from 2026-10-01.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 2: `MatchUpdate` and `GameKey`

**Files:**
- Create: `polyoxide-sports/src/update.rs`
- Modify: `polyoxide-sports/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-sports/src/update.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::fixtures;

    #[test]
    fn every_fixture_parses() {
        for (name, frame) in fixtures::ALL {
            MatchUpdate::from_json(frame).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn every_fixture_round_trips_losslessly() {
        // Catches an absent optional field serialising as `null`, and an
        // unknown key lost instead of kept in `extra`.
        for (name, frame) in fixtures::ALL {
            let parsed = MatchUpdate::from_json(frame).unwrap();
            let original: Value = serde_json::from_str(frame).unwrap();
            assert_eq!(
                serde_json::to_value(&parsed).unwrap(),
                original,
                "{name} changed on a round trip"
            );
        }
    }

    #[test]
    fn every_field_reads_its_own_key() {
        // Distinct values per key, so a wrong `rename` or a swapped field
        // fails here even though every fixture still parses.
        let frame = json!({
            "leagueAbbreviation": "league", "score": "score", "period": "period",
            "live": true, "ended": false, "gameId": 7, "metadataGameId": "meta",
            "homeTeam": "home", "awayTeam": "away", "status": "status",
            "elapsed": "elapsed", "finishedTimestamp": "finished",
            "eventState": {"type": "state"}
        });
        let update: MatchUpdate = serde_json::from_value(frame).unwrap();
        assert_eq!(update.league_abbreviation, "league");
        assert_eq!(update.score, "score");
        assert_eq!(update.period, "period");
        assert!(update.live);
        assert!(!update.ended);
        assert_eq!(update.game_id, Some(7));
        assert_eq!(update.metadata_game_id.as_deref(), Some("meta"));
        assert_eq!(update.home_team.as_deref(), Some("home"));
        assert_eq!(update.away_team.as_deref(), Some("away"));
        assert_eq!(update.status.as_deref(), Some("status"));
        assert_eq!(update.elapsed.as_deref(), Some("elapsed"));
        assert_eq!(update.finished_timestamp.as_deref(), Some("finished"));
        assert_eq!(update.event_state, Some(json!({"type": "state"})));
        assert!(
            update.extra.is_empty(),
            "a modelled key fell through to extra: {:?}",
            update.extra.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn live_and_ended_are_not_swapped() {
        let update = MatchUpdate::from_json(
            r#"{"leagueAbbreviation":"l","score":"s","period":"p","live":false,"ended":true}"#,
        )
        .unwrap();
        assert!(!update.live);
        assert!(update.ended);
    }

    #[test]
    fn a_frame_missing_a_required_field_is_rejected() {
        for field in ["leagueAbbreviation", "score", "period", "live", "ended"] {
            let mut frame: Value = serde_json::from_str(fixtures::SOCCER).unwrap();
            frame.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<MatchUpdate>(frame).is_err(),
                "parsed a frame without {field}"
            );
        }
    }

    #[test]
    fn unmodelled_keys_are_kept() {
        let update = MatchUpdate::from_json(
            r#"{"leagueAbbreviation":"nfl","score":"14-7","period":"Q2","live":true,
                "ended":false,"turn":"sea","somethingNew":42}"#,
        )
        .unwrap();
        assert_eq!(update.extra["turn"], "sea");
        assert_eq!(update.extra["somethingNew"], 42);
    }

    #[test]
    fn a_numeric_game_is_keyed_by_its_game_id() {
        let update = MatchUpdate::from_json(fixtures::SOCCER).unwrap();
        assert_eq!(update.key(), Some(GameKey::Game(90106111)));
    }

    #[test]
    fn a_cricket_game_is_keyed_by_its_metadata_id() {
        let update = MatchUpdate::from_json(fixtures::CRICKET).unwrap();
        assert_eq!(
            update.key(),
            Some(GameKey::Metadata("id2703680373085574".into()))
        );
    }

    #[test]
    fn the_numeric_id_wins_when_both_are_present() {
        let update = MatchUpdate::from_json(
            r#"{"leagueAbbreviation":"l","score":"s","period":"p","live":true,"ended":false,
                "gameId":5,"metadataGameId":"id5"}"#,
        )
        .unwrap();
        assert_eq!(update.key(), Some(GameKey::Game(5)));
    }

    #[test]
    fn a_frame_with_neither_id_has_no_key_but_still_parses() {
        let update = MatchUpdate::from_json(
            r#"{"leagueAbbreviation":"l","score":"s","period":"p","live":true,"ended":false}"#,
        )
        .unwrap();
        assert_eq!(update.key(), None);
    }

    #[test]
    fn a_key_displays_as_the_wire_sent_it() {
        assert_eq!(GameKey::Game(1712005).to_string(), "1712005");
        assert_eq!(
            GameKey::Metadata("id2704098174740616".into()).to_string(),
            "id2704098174740616"
        );
    }

    #[test]
    fn a_repeated_frame_compares_equal_and_a_changed_one_does_not() {
        let first = MatchUpdate::from_json(fixtures::SOCCER).unwrap();
        let repeat = MatchUpdate::from_json(fixtures::SOCCER).unwrap();
        assert_eq!(first, repeat);
        let mut changed: Value = serde_json::from_str(fixtures::SOCCER).unwrap();
        changed["score"] = json!("3-1");
        let changed: MatchUpdate = serde_json::from_value(changed).unwrap();
        assert_ne!(first, changed);
    }
}
```

Add to `polyoxide-sports/src/lib.rs`, after `#![warn(missing_docs)]`:

```rust
pub mod update;

pub use update::{GameKey, MatchUpdate};
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p polyoxide-sports update`
Expected: compile error, `cannot find type MatchUpdate in this scope`.

- [ ] **Step 3: Write the implementation above the test module in `update.rs`**

```rust
//! The match update frame and the identifier that spans every sport.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The full current state of one match, as the sports feed sends it.
///
/// # Field requiredness
///
/// Modelled on frames captured on 2026-07-25 (229 frames) and 2026-10-01
/// (121 frames). Only the five fields present on every frame are required.
/// Cricket frames carry no `gameId`, `homeTeam`, `awayTeam` or `status`, and
/// identify the match with `metadataGameId` instead. Use
/// [`key`](Self::key) rather than either id field directly.
///
/// # Frames are state, not events
///
/// The server re-sends unchanged state on a timer: every 20 seconds per live
/// esports game, and every 30 to 90 seconds for tennis. Consecutive frames
/// for one game are often identical, so a frame is not a change. Compare it
/// with `==` against the last frame for the same [`GameKey`] to drop
/// repeats.
///
/// The frame saying a match ended is sent once. A consumer that is
/// disconnected at that moment never sees it; the supervised stream's
/// `Event::Reconnected` says when that may have happened.
///
/// Fields this type does not model are kept in [`extra`](Self::extra), and
/// serialising a parsed frame reproduces it exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MatchUpdate {
    /// League or competition label, e.g. `"atp"`, `"lol"`, `"cricket"`. Free
    /// text, not a code: `"wta challenger"` has been seen.
    pub league_abbreviation: String,
    /// Current score. The format is sport-specific: `"2-1"` for soccer,
    /// `"4-6, 1-2"` for tennis, `"000-000|0-1|Bo3"` for esports.
    pub score: String,
    /// Current period, e.g. `"2H"`, `"S2"`, `"1/3"`, `"Live"`, `"FT"`.
    pub period: String,
    /// Whether the match is in progress.
    pub live: bool,
    /// Whether the match has finished.
    pub ended: bool,
    /// Numeric match id. Absent on cricket. Gamma's events list filters on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<u64>,
    /// Cricket's string match id, sent in place of [`game_id`](Self::game_id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_game_id: Option<String>,
    /// Home team or first player. Absent on cricket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_team: Option<String>,
    /// Away team or second player. Absent on cricket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub away_team: Option<String>,
    /// Venue status string. Casing is not normalised upstream: `"InProgress"`,
    /// `"inprogress"`, `"running"` and `"finished"` have all been seen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Elapsed time within the period, as `"MM:SS"` or minutes. Often absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<String>,
    /// When the match ended. Present only once [`ended`](Self::ended) is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_timestamp: Option<String>,
    /// Per-sport detail whose shape varies by sport, kept as raw JSON. Seen on
    /// soccer and tennis in July 2026, and on no frame in October 2026.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_state: Option<Value>,
    /// Every key this type does not model, kept rather than dropped.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl MatchUpdate {
    /// Parse one text frame.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// The match's identifier, whichever form the frame carries.
    ///
    /// [`GameKey::Game`] wins when a frame carries both, because only that
    /// form can be looked up in gamma. `None` only for a frame carrying
    /// neither, which no capture has shown.
    pub fn key(&self) -> Option<GameKey> {
        match (self.game_id, &self.metadata_game_id) {
            (Some(id), _) => Some(GameKey::Game(id)),
            (None, Some(id)) => Some(GameKey::Metadata(id.clone())),
            (None, None) => None,
        }
    }
}

/// One identifier across every sport.
///
/// Only the numeric form can be reconciled. After a reconnect, look up a game
/// that may have ended during the gap with
/// `gamma.events().list().game_id([id as i64])`: the event comes back with its
/// `ended` flag and final `score`. Gamma refuses cricket's string form with
/// `invalid integer`, so a cricket game cannot be reconciled that way.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameKey {
    /// The numeric `gameId`.
    Game(u64),
    /// Cricket's `metadataGameId`.
    Metadata(String),
}

impl fmt::Display for GameKey {
    /// The id as the wire sent it: decimal for [`Game`](Self::Game), verbatim
    /// for [`Metadata`](Self::Metadata).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameKey::Game(id) => write!(f, "{id}"),
            GameKey::Metadata(id) => f.write_str(id),
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p polyoxide-sports`
Expected: all pass, 15 tests.

- [ ] **Step 5: Prove the round-trip test catches a lost `skip_serializing_if`**

Delete `skip_serializing_if = "Option::is_none"` from the `elapsed` field only, leaving `#[serde(default)]`. Run `cargo test -p polyoxide-sports every_fixture_round_trips_losslessly`.
Expected: FAIL with `tennis_event_state changed on a round trip`, the first fixture without `elapsed`, because `"elapsed": null` appears. Restore the attribute and rerun to PASS.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-sports/src
git commit -F - <<'MSG'
feat(sports): MatchUpdate and GameKey, pinned to captured frames

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 3: `SportsError`

**Files:**
- Create: `polyoxide-sports/src/error.rs`
- Modify: `polyoxide-sports/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-sports/src/error.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_cross_threads() {
        // The CLI turns this into an eyre::Report, which needs Send + Sync.
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SportsError>();
    }

    #[test]
    fn a_stale_error_says_pings_count() {
        let text = SportsError::Stale {
            after: Duration::from_secs(45),
        }
        .to_string();
        assert!(text.contains("45s") && text.contains("pings included"), "{text}");
    }

    #[test]
    fn a_close_error_names_its_code_and_reason() {
        let text = SportsError::Closed {
            code: Some(1001),
            reason: "going away".into(),
        }
        .to_string();
        assert!(text.contains("1001") && text.contains("going away"), "{text}");
    }
}
```

Add `pub mod error;` to `lib.rs` above `pub mod update;`, and `pub use error::SportsError;` above the `update` re-export.

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p polyoxide-sports error`
Expected: compile error, `cannot find type SportsError`.

- [ ] **Step 3: Write the implementation above the test module**

```rust
//! The crate's error type.

use std::time::Duration;

use tokio_tungstenite::tungstenite;

/// Everything that can go wrong on the sports feed.
///
/// On the supervised stream only [`Decode`](Self::Decode) reaches the caller
/// as an `Err`. Every other variant arrives inside `Event::Disconnected`,
/// after the stream has already acted on it.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SportsError {
    /// The WebSocket handshake failed.
    #[error("could not connect to the sports feed: {source}")]
    Connect {
        /// What the transport reported.
        #[source]
        source: Box<tungstenite::Error>,
    },
    /// The handshake did not finish within the connect timeout.
    #[error("connecting to the sports feed took longer than {after:?}")]
    ConnectTimeout {
        /// The timeout that elapsed.
        after: Duration,
    },
    /// The server closed the connection, or the stream ended.
    #[error("the sports feed closed the connection (code {code:?}, reason {reason:?})")]
    Closed {
        /// The close code, when the server sent a close frame.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A read failed on an open connection.
    #[error("the sports feed connection failed: {source}")]
    Transport {
        /// What the transport reported.
        #[source]
        source: Box<tungstenite::Error>,
    },
    /// Nothing arrived, protocol pings included, within the staleness limit.
    #[error("nothing received from the sports feed for {after:?}, pings included")]
    Stale {
        /// The staleness limit that elapsed.
        after: Duration,
    },
    /// A text frame did not parse as a match update.
    #[error("a sports frame did not parse: {source}")]
    Decode {
        /// The frame as received.
        raw: String,
        /// Why it did not parse.
        #[source]
        source: serde_json::Error,
    },
}
```

- [ ] **Step 4: Run to see it pass**

Run: `cargo test -p polyoxide-sports`
Expected: all pass, 18 tests.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-sports/src
git commit -F - <<'MSG'
feat(sports): SportsError

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 4: The bare tier

**Files:**
- Create: `polyoxide-sports/src/client.rs`
- Modify: `polyoxide-sports/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-sports/src/client.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};

    use super::*;
    use crate::fixtures;

    #[test]
    fn the_default_url_is_the_sports_host() {
        assert_eq!(SPORTS_WS_URL, "wss://sports-api.polymarket.com/ws");
    }

    #[test]
    fn a_text_frame_is_an_update() {
        let Inbound::Update(update) = classify(Message::Text(fixtures::SOCCER.into())) else {
            panic!("a captured frame was not classified as an update");
        };
        assert_eq!(update.league_abbreviation, "kor");
    }

    #[test]
    fn a_bad_text_frame_keeps_its_raw_text() {
        let Inbound::Undecodable(SportsError::Decode { raw, .. }) =
            classify(Message::Text("not json".into()))
        else {
            panic!("a bad frame was not reported as undecodable");
        };
        assert_eq!(raw, "not json");
    }

    #[test]
    fn control_and_binary_frames_are_proof_of_life() {
        for message in [
            Message::Ping(b"p".to_vec().into()),
            Message::Pong(b"p".to_vec().into()),
            Message::Binary(b"b".to_vec().into()),
        ] {
            assert!(matches!(classify(message), Inbound::Alive));
        }
    }

    #[test]
    fn a_close_frame_keeps_its_code_and_reason() {
        let frame = CloseFrame {
            code: CloseCode::Away,
            reason: "bye".into(),
        };
        let Inbound::Closed(SportsError::Closed { code, reason }) =
            classify(Message::Close(Some(frame)))
        else {
            panic!("a close frame was not classified as closed");
        };
        assert_eq!(code, Some(1001));
        assert_eq!(reason, "bye");
    }

    #[test]
    fn a_bare_close_has_no_code() {
        let Inbound::Closed(SportsError::Closed { code, reason }) =
            classify(Message::Close(None))
        else {
            panic!("a bare close was not classified as closed");
        };
        assert_eq!(code, None);
        assert!(reason.is_empty());
    }

    #[test]
    fn the_bare_stream_can_move_between_tasks() {
        fn assert_send_unpin<T: Send + Unpin>() {}
        assert_send_unpin::<SportsWs>();
    }
}
```

Add to `lib.rs`: `pub mod client;` above `pub mod error;`, and `pub use client::{SportsWs, SPORTS_WS_URL};` above the other re-exports.

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p polyoxide-sports client`
Expected: compile error, `cannot find value SPORTS_WS_URL`.

- [ ] **Step 3: Write the implementation above the test module**

```rust
//! The bare tier, and the connect and classify steps both tiers share.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::CloseFrame, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{error::SportsError, update::MatchUpdate};

/// The production endpoint.
pub const SPORTS_WS_URL: &str = "wss://sports-api.polymarket.com/ws";

/// How long one handshake may take before it is abandoned.
pub(crate) const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection.
pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Make sure rustls has a default `CryptoProvider` before opening a connection.
///
/// A deliberate twin of the same function in `polyoxide-clob`,
/// `polyoxide-rtds` and `polyoxide-perps`. `tokio-tungstenite` builds its TLS
/// config from the process-wide default provider, and rustls installs one
/// automatically only when exactly one backend feature is enabled. With
/// `ring` and `aws-lc-rs` both in a consumer's graph it installs neither and
/// panics inside `connect_async`. `install_default` returns `Err` when a
/// provider is already set, so crates racing is a no-op.
fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Open one connection, bounded by `connect_timeout`.
///
/// Takes the URL by value so the supervised tier can box the future.
pub(crate) async fn open(url: String, connect_timeout: Duration) -> Result<Socket, SportsError> {
    ensure_crypto_provider();
    match tokio::time::timeout(connect_timeout, connect_async(url.as_str())).await {
        Ok(Ok((socket, _response))) => Ok(socket),
        Ok(Err(source)) => Err(SportsError::Connect {
            source: Box::new(source),
        }),
        Err(_) => Err(SportsError::ConnectTimeout {
            after: connect_timeout,
        }),
    }
}

/// What one inbound message means. Both tiers classify through this, so they
/// cannot disagree about a frame.
pub(crate) enum Inbound {
    /// A match update, boxed because it is several times larger than the
    /// other variants.
    Update(Box<MatchUpdate>),
    /// A text frame that did not parse.
    Undecodable(SportsError),
    /// A control or binary frame: proof of life with nothing to yield.
    Alive,
    /// The server closed the connection.
    Closed(SportsError),
}

/// Classify one inbound message.
pub(crate) fn classify(message: Message) -> Inbound {
    match message {
        Message::Text(text) => match MatchUpdate::from_json(&text) {
            Ok(update) => Inbound::Update(Box::new(update)),
            Err(source) => Inbound::Undecodable(SportsError::Decode {
                raw: text.to_string(),
                source,
            }),
        },
        Message::Close(frame) => Inbound::Closed(closed(frame)),
        Message::Binary(bytes) => {
            tracing::debug!(len = bytes.len(), "skipping a binary frame on the sports feed");
            Inbound::Alive
        }
        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => Inbound::Alive,
    }
}

/// The error for a connection that closed, with or without a close frame.
pub(crate) fn closed(frame: Option<CloseFrame>) -> SportsError {
    match frame {
        Some(frame) => SportsError::Closed {
            code: Some(u16::from(frame.code)),
            reason: frame.reason.to_string(),
        },
        None => SportsError::Closed {
            code: None,
            reason: String::new(),
        },
    }
}

/// One connection to the sports feed. Ends when the connection does.
///
/// For a feed that reconnects on its own, use `SportsWsBuilder`.
///
/// The server sends a protocol ping every 15 seconds. The transport queues
/// the pong when it reads the ping and sends it at the start of the next
/// read, so pongs go out as long as the stream is being polled.
///
/// # Example
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_sports::SportsWs;
///
/// # async fn run() -> Result<(), polyoxide_sports::SportsError> {
/// let mut feed = SportsWs::connect().await?;
/// while let Some(update) = feed.next().await {
///     let update = update?;
///     println!("{} {} {}", update.league_abbreviation, update.score, update.period);
/// }
/// # Ok(())
/// # }
/// ```
pub struct SportsWs {
    socket: Socket,
    finished: bool,
}

impl SportsWs {
    /// Connect to the production feed.
    pub async fn connect() -> Result<Self, SportsError> {
        Self::connect_to(SPORTS_WS_URL).await
    }

    /// Connect to another endpoint, such as a local test server.
    pub async fn connect_to(url: &str) -> Result<Self, SportsError> {
        let socket = open(url.to_owned(), DEFAULT_CONNECT_TIMEOUT).await?;
        Ok(Self {
            socket,
            finished: false,
        })
    }
}

impl Stream for SportsWs {
    type Item = Result<MatchUpdate, SportsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        loop {
            let message = match self.socket.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(source))) => {
                    self.finished = true;
                    return Poll::Ready(Some(Err(SportsError::Transport {
                        source: Box::new(source),
                    })));
                }
                Poll::Ready(None) => {
                    self.finished = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            };
            match classify(message) {
                Inbound::Update(update) => return Poll::Ready(Some(Ok(*update))),
                Inbound::Undecodable(error) => return Poll::Ready(Some(Err(error))),
                // Reading again is what sends the pong for a ping just read.
                Inbound::Alive => continue,
                Inbound::Closed(reason) => {
                    tracing::debug!(%reason, "the sports feed closed");
                    self.finished = true;
                    return Poll::Ready(None);
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run to see it pass, and lint**

Run: `cargo test -p polyoxide-sports && cargo clippy -p polyoxide-sports --all-targets --all-features -- -D warnings`
Expected: all tests pass, 25 tests; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-sports/src
git commit -F - <<'MSG'
feat(sports): bare SportsWs stream and the shared frame classifier

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 5: The scripted test server

**Files:**
- Create: `polyoxide-sports/src/test_server.rs`
- Modify: `polyoxide-sports/src/lib.rs`

This server is test infrastructure. Tasks 6, 8 and 9 exercise every path through it.

- [ ] **Step 1: Write `polyoxide-sports/src/test_server.rs`**

```rust
//! A local WebSocket server that plays a script per connection, for the
//! offline tests here and in `polyoxide-cli`. Behind `test-server`; not for
//! consumers.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    time::Instant,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Accept the TCP connection, then drop it without a handshake.
    pub reject_handshake: bool,
    /// Accept the TCP connection and never answer the handshake.
    pub stall_handshake: bool,
    /// Messages to send right after the handshake, in order.
    pub send: Vec<Message>,
    /// Send a close frame after `send`. Otherwise hold the connection open.
    pub close_after: bool,
    /// While holding the connection open, send a protocol ping this often.
    pub ping_every: Option<Duration>,
}

impl Script {
    /// Hold the connection open and send nothing at all.
    pub fn silent() -> Self {
        Self::default()
    }

    /// Send these text frames, then hold the connection open in silence.
    pub fn frames(frames: &[&str]) -> Self {
        Self {
            send: frames.iter().map(|frame| Message::Text((*frame).into())).collect(),
            ..Self::default()
        }
    }

    /// Hold the connection open, sending only protocol pings.
    pub fn pings_only(every: Duration) -> Self {
        Self {
            ping_every: Some(every),
            ..Self::default()
        }
    }

    /// Complete the handshake, then close at once.
    pub fn close_at_once() -> Self {
        Self {
            close_after: true,
            ..Self::default()
        }
    }

    /// Drop the TCP connection without a handshake.
    pub fn reject() -> Self {
        Self {
            reject_handshake: true,
            ..Self::default()
        }
    }

    /// Accept the TCP connection and never answer the handshake.
    pub fn stall() -> Self {
        Self {
            stall_handshake: true,
            ..Self::default()
        }
    }

    /// Close after sending, instead of holding the connection open.
    pub fn then_close(mut self) -> Self {
        self.close_after = true;
        self
    }
}

#[derive(Default)]
struct Recorder {
    accepted_at: Mutex<Vec<Instant>>,
    handshakes: AtomicUsize,
    pongs: Mutex<Vec<Vec<u8>>>,
    client_ended: AtomicUsize,
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL to connect to.
    pub url: String,
    recorder: Arc<Recorder>,
}

impl ScriptedServer {
    /// Start a server that applies `scripts[n]` to the n-th connection,
    /// repeating the last script for every connection after it.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("ws://{}", listener.local_addr().expect("local address"));
        let recorder = Arc::new(Recorder::default());
        let shared = Arc::clone(&recorder);
        tokio::spawn(async move {
            let mut index = 0;
            while let Ok((stream, _)) = listener.accept().await {
                shared.accepted_at.lock().unwrap().push(Instant::now());
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or_default();
                index += 1;
                let recorder = Arc::clone(&shared);
                tokio::spawn(async move {
                    let _ = serve(stream, script, recorder).await;
                });
            }
        });
        Self { url, recorder }
    }

    /// Connections accepted, counted at TCP accept, before any handshake.
    pub fn connection_count(&self) -> usize {
        self.recorder.accepted_at.lock().unwrap().len()
    }

    /// When each connection was accepted, in order.
    pub fn accepted_at(&self) -> Vec<Instant> {
        self.recorder.accepted_at.lock().unwrap().clone()
    }

    /// Handshakes completed.
    pub fn handshake_count(&self) -> usize {
        self.recorder.handshakes.load(Ordering::SeqCst)
    }

    /// The payload of every pong received, in order, across connections.
    pub fn pongs(&self) -> Vec<Vec<u8>> {
        self.recorder.pongs.lock().unwrap().clone()
    }

    /// Held connections the client ended, by a close frame, end of stream,
    /// or a read error.
    pub fn client_ended_count(&self) -> usize {
        self.recorder.client_ended.load(Ordering::SeqCst)
    }

    /// Poll until `predicate` holds, or panic naming `label` after five
    /// seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for {label}");
    }
}

async fn serve(
    stream: TcpStream,
    script: Script,
    recorder: Arc<Recorder>,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    if script.reject_handshake {
        drop(stream);
        return Ok(());
    }
    if script.stall_handshake {
        // Hold the socket without reading it until the test ends.
        let _held = stream;
        std::future::pending::<()>().await;
        return Ok(());
    }
    let mut ws = accept_async(stream).await?;
    recorder.handshakes.fetch_add(1, Ordering::SeqCst);
    for message in script.send {
        ws.send(message).await?;
    }
    if script.close_after {
        let _ = ws.close(None).await;
        return Ok(());
    }
    let mut pings = script
        .ping_every
        .map(|every| tokio::time::interval_at(Instant::now() + every, every));
    let mut sequence: u32 = 0;
    loop {
        tokio::select! {
            _ = async {
                match pings.as_mut() {
                    Some(interval) => {
                        interval.tick().await;
                    }
                    None => std::future::pending::<()>().await,
                }
            } => {
                sequence += 1;
                ws.send(Message::Ping(sequence.to_be_bytes().to_vec().into())).await?;
            }
            message = ws.next() => match message {
                Some(Ok(Message::Pong(payload))) => {
                    recorder.pongs.lock().unwrap().push(payload.to_vec());
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                    recorder.client_ended.fetch_add(1, Ordering::SeqCst);
                    return Ok(());
                }
                Some(Ok(_)) => {}
            },
        }
    }
}
```

- [ ] **Step 2: Export it from `lib.rs`**

Add after the `fixtures` module declaration:

```rust
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;
```

- [ ] **Step 3: Build and lint with the feature**

Run: `cargo clippy -p polyoxide-sports --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-sports/src
git commit -F - <<'MSG'
test(sports): scripted local WebSocket server

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 6: The bare tier against the scripted server

**Files:**
- Create: `polyoxide-sports/tests/bare.rs`
- Modify: `polyoxide-sports/Cargo.toml`

- [ ] **Step 1: Write the tests**

```rust
//! The bare tier against the scripted server.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_sports::{
    fixtures,
    test_server::{Script, ScriptedServer},
    SportsError, SportsWs,
};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

const WINDOW: Duration = Duration::from_secs(3);

fn text(frame: &str) -> Message {
    Message::Text(frame.into())
}

#[tokio::test]
async fn yields_updates_and_skips_protocol_pings() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Ping(b"p".to_vec().into()), text(fixtures::SOCCER)],
        ..Script::silent()
    }])
    .await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    let update = timeout(WINDOW, feed.next())
        .await
        .expect("an update within the window")
        .expect("the stream is open")
        .expect("the frame parses");
    assert_eq!(update.league_abbreviation, "kor");
}

#[tokio::test]
async fn answers_a_protocol_ping_with_a_matching_pong() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Ping(b"p1".to_vec().into())],
        ..Script::silent()
    }])
    .await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    let reader = tokio::spawn(async move { while feed.next().await.is_some() {} });
    server
        .wait_for("a pong", |s| s.pongs() == [b"p1".to_vec()])
        .await;
    reader.abort();
}

#[tokio::test]
async fn reports_a_bad_frame_and_keeps_reading() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![text("not json"), text(fixtures::SOCCER)],
        ..Script::silent()
    }])
    .await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    match timeout(WINDOW, feed.next()).await.unwrap() {
        Some(Err(SportsError::Decode { raw, .. })) => assert_eq!(raw, "not json"),
        other => panic!("expected a decode error, got {other:?}"),
    }
    let update = timeout(WINDOW, feed.next()).await.unwrap().unwrap().unwrap();
    assert_eq!(update.league_abbreviation, "kor");
}

#[tokio::test]
async fn ends_when_the_server_closes() {
    let server =
        ScriptedServer::start(vec![Script::frames(&[fixtures::SOCCER]).then_close()]).await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    assert!(matches!(
        timeout(WINDOW, feed.next()).await.unwrap(),
        Some(Ok(_))
    ));
    assert!(timeout(WINDOW, feed.next()).await.unwrap().is_none());
    assert!(
        timeout(WINDOW, feed.next()).await.unwrap().is_none(),
        "the stream yielded again after ending"
    );
}

#[tokio::test]
async fn dropping_the_stream_closes_the_socket() {
    let server = ScriptedServer::start(vec![Script::silent()]).await;
    let feed = SportsWs::connect_to(&server.url).await.unwrap();
    server
        .wait_for("the handshake", |s| s.handshake_count() == 1)
        .await;
    drop(feed);
    server
        .wait_for("the client to end the connection", |s| {
            s.client_ended_count() == 1
        })
        .await;
}
```

- [ ] **Step 2: Gate the file on the feature, then run it**

In `polyoxide-sports/Cargo.toml`, replace the comment `# Tasks 6 and 8 add `[[test]]` entries here...` (both lines) with:

```toml
# These import `test_server` and `fixtures`, which exist only behind the
# feature. `required-features` makes a plain `cargo test --all-targets` skip
# them instead of failing to build them.
[[test]]
name = "bare"
required-features = ["test-server"]
```

Run: `cargo test -p polyoxide-sports --features test-server --test bare`
Expected: 5 passed. Then `cargo test -p polyoxide-sports --all-targets` (no features) must also pass, skipping `bare`.

- [ ] **Step 3: Prove the pong test catches a read loop that stops after a ping**

In `polyoxide-sports/src/client.rs`, inside `SportsWs::poll_next`, change `Inbound::Alive => continue,` to `Inbound::Alive => return Poll::Pending,`. Run `cargo test -p polyoxide-sports --features test-server --test bare answers_a_protocol_ping`.
Expected: FAIL with `timed out waiting for a pong`. Revert the change and rerun the whole file to PASS.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-sports/tests/bare.rs polyoxide-sports/Cargo.toml
git commit -F - <<'MSG'
test(sports): bare tier against the scripted server

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 7: The supervised tier

**Files:**
- Create: `polyoxide-sports/src/supervised.rs`
- Modify: `polyoxide-sports/src/lib.rs`

- [ ] **Step 1: Write the failing unit tests**

Create `polyoxide-sports/src/supervised.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn backoff_doubles_up_to_its_ceiling() {
        let mut backoff = Backoff::new(MS * 100, MS * 1000);
        let delays: Vec<Duration> = (0..6).map(|_| backoff.take()).collect();
        assert_eq!(
            delays,
            [MS * 100, MS * 200, MS * 400, MS * 800, MS * 1000, MS * 1000]
        );
    }

    #[test]
    fn backoff_resets_only_after_a_connection_that_received_something() {
        let mut backoff = Backoff::new(MS * 100, MS * 1000);
        backoff.take();
        backoff.take();
        backoff.after_connection_ended(false);
        assert_eq!(backoff.take(), MS * 400, "a silent connection reset the schedule");
        backoff.after_connection_ended(true);
        assert_eq!(backoff.take(), MS * 100);
    }

    #[test]
    fn an_initial_delay_above_the_ceiling_is_clamped() {
        let mut backoff = Backoff::new(MS * 5000, MS * 1000);
        assert_eq!(backoff.take(), MS * 1000);
    }

    #[test]
    fn the_builder_defaults_are_the_documented_ones() {
        let builder = SportsWsBuilder::new();
        assert_eq!(builder.url, SPORTS_WS_URL);
        assert_eq!(builder.stale_after, Duration::from_secs(45));
        assert_eq!(builder.initial_backoff, Duration::from_millis(500));
        assert_eq!(builder.max_backoff, Duration::from_secs(60));
        assert_eq!(builder.connect_timeout, Duration::from_secs(10));
    }

    #[test]
    fn the_supervised_stream_can_move_between_tasks() {
        fn assert_send_unpin<T: Send + Unpin>() {}
        assert_send_unpin::<SupervisedSportsWs>();
    }
}
```

Add to `lib.rs`: `pub mod supervised;` after `pub mod error;`, and `pub use supervised::{Event, SportsWsBuilder, SupervisedSportsWs};` after the `error` re-export.

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p polyoxide-sports supervised`
Expected: compile error, `cannot find type Backoff`.

- [ ] **Step 3: Write the implementation above the test module**

```rust
//! The supervised tier: a feed that reconnects for as long as it is held,
//! and says when its scores may be stale.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::time::{sleep, Instant, Sleep};

use crate::{
    client::{classify, closed, open, Inbound, Socket, DEFAULT_CONNECT_TIMEOUT, SPORTS_WS_URL},
    error::SportsError,
    update::MatchUpdate,
};

/// Three missed server pings; the server sends one every 15 seconds.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(45);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);

/// What a supervised feed yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// A match update. Boxed because it is several times larger than the
    /// other variants; it dereferences to [`MatchUpdate`].
    Update(Box<MatchUpdate>),
    /// The connection was lost. Scores are stale from here until
    /// [`Event::Reconnected`].
    Disconnected {
        /// Why the connection was given up.
        reason: SportsError,
    },
    /// A new connection is up.
    ///
    /// The frame saying a match ended is sent once, so games that ended
    /// during the gap were not re-sent. Reconcile them through gamma by
    /// [`GameKey::Game`](crate::GameKey::Game). Cricket games cannot be
    /// reconciled.
    Reconnected,
}

/// The reconnect delay schedule: doubling to a ceiling, and back to the start
/// after a connection that received anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

impl Backoff {
    fn new(initial: Duration, max: Duration) -> Self {
        let initial = initial.min(max);
        Self {
            initial,
            max,
            next: initial,
        }
    }

    /// The delay to wait now. The one after it doubles.
    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(self.max);
        delay
    }

    /// A connection ended. One that received anything resets the schedule.
    /// One that received nothing, such as a server that accepts and closes at
    /// once, keeps it growing.
    fn after_connection_ended(&mut self, received: bool) {
        if received {
            self.next = self.initial;
        }
    }
}

/// Builder for a [`SupervisedSportsWs`].
///
/// ```no_run
/// use std::time::Duration;
/// use polyoxide_sports::SportsWsBuilder;
///
/// # async fn run() -> Result<(), polyoxide_sports::SportsError> {
/// let feed = SportsWsBuilder::new()
///     .stale_after(Duration::from_secs(60))
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SportsWsBuilder {
    url: String,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
    connect_timeout: Duration,
}

impl Default for SportsWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SportsWsBuilder {
    /// The production URL, a 45 s staleness limit, backoff from 500 ms to
    /// 60 s, and a 10 s connect timeout.
    pub fn new() -> Self {
        Self {
            url: SPORTS_WS_URL.to_owned(),
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }

    /// Connect somewhere other than production, such as a local test server.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// How long a connection may go without receiving anything, protocol
    /// pings included, before it is treated as dead.
    ///
    /// Keep this above the server's 15-second ping interval. Data cannot set
    /// it: when nothing is live anywhere, no data arrives at all.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay bounds. The delay doubles from `initial` up to `max`,
    /// and returns to `initial` after a connection that received anything.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// How long one connection attempt may take.
    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    /// Make the first connection and start supervising.
    ///
    /// Fails if the first connection fails, so a wrong URL or a network with
    /// no route surfaces at once. Every later failure is retried.
    pub async fn connect(self) -> Result<SupervisedSportsWs, SportsError> {
        let socket = open(self.url.clone(), self.connect_timeout).await?;
        Ok(SupervisedSportsWs {
            backoff: Backoff::new(self.initial_backoff, self.max_backoff),
            state: State::reading(Box::new(socket), self.stale_after),
            config: self,
        })
    }
}

type Attempt = Pin<Box<dyn Future<Output = Result<Socket, SportsError>> + Send>>;

enum State {
    /// Reading an open connection.
    Reading {
        socket: Box<Socket>,
        stale: Pin<Box<Sleep>>,
        received: bool,
    },
    /// Waiting out a backoff delay.
    Waiting(Pin<Box<Sleep>>),
    /// A connection attempt in flight.
    Connecting(Attempt),
}

impl State {
    fn reading(socket: Box<Socket>, stale_after: Duration) -> Self {
        State::Reading {
            socket,
            stale: Box::pin(sleep(stale_after)),
            received: false,
        }
    }
}

/// One iteration's outcome, decided while the state is borrowed and acted on
/// after the borrow ends.
enum Step {
    Pending,
    Yield(Result<Event, SportsError>),
    Lost(SportsError),
    Attempt,
    Connected(Box<Socket>),
    AttemptFailed(SportsError),
}

/// A sports feed that reconnects for as long as it is held.
///
/// Yields [`Event`]s. Only an undecodable frame arrives as `Err`, and the
/// stream carries on after it. Every outage yields one
/// [`Event::Disconnected`] when the connection is lost and one
/// [`Event::Reconnected`] when a new one is up, however many attempts that
/// takes. The stream never ends while held, and dropping it closes the
/// connection.
///
/// A connection that receives nothing, protocol pings included, for the
/// staleness limit is treated as dead.
///
/// No background task runs: all the work happens inside `poll_next`. So the
/// server's pings are answered only while the stream is being polled. A
/// caller that stops polling for long enough will be dropped by the server,
/// and will see a disconnect and a reconnect when it resumes.
pub struct SupervisedSportsWs {
    config: SportsWsBuilder,
    backoff: Backoff,
    state: State,
}

impl SupervisedSportsWs {
    /// Give up the current connection and schedule the next attempt.
    fn lose(&mut self, reason: SportsError) -> Event {
        let received = matches!(self.state, State::Reading { received: true, .. });
        self.backoff.after_connection_ended(received);
        let delay = self.backoff.take();
        tracing::warn!(%reason, ?delay, "lost the sports feed; reconnecting");
        // Replacing the state drops the old socket.
        self.state = State::Waiting(Box::pin(sleep(delay)));
        Event::Disconnected { reason }
    }
}

impl Stream for SupervisedSportsWs {
    type Item = Result<Event, SportsError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            let step = match &mut this.state {
                State::Reading {
                    socket,
                    stale,
                    received,
                } => poll_reading(socket, stale, received, this.config.stale_after, cx),
                State::Waiting(delay) => match delay.as_mut().poll(cx) {
                    Poll::Ready(()) => Step::Attempt,
                    Poll::Pending => Step::Pending,
                },
                State::Connecting(attempt) => match attempt.as_mut().poll(cx) {
                    Poll::Ready(Ok(socket)) => Step::Connected(Box::new(socket)),
                    Poll::Ready(Err(error)) => Step::AttemptFailed(error),
                    Poll::Pending => Step::Pending,
                },
            };
            match step {
                Step::Pending => return Poll::Pending,
                Step::Yield(item) => return Poll::Ready(Some(item)),
                Step::Lost(reason) => return Poll::Ready(Some(Ok(this.lose(reason)))),
                Step::Attempt => {
                    this.state = State::Connecting(Box::pin(open(
                        this.config.url.clone(),
                        this.config.connect_timeout,
                    )));
                }
                Step::Connected(socket) => {
                    tracing::info!("reconnected to the sports feed");
                    this.state = State::reading(socket, this.config.stale_after);
                    return Poll::Ready(Some(Ok(Event::Reconnected)));
                }
                Step::AttemptFailed(error) => {
                    let delay = this.backoff.take();
                    tracing::warn!(%error, ?delay, "sports feed reconnect failed; retrying");
                    this.state = State::Waiting(Box::pin(sleep(delay)));
                }
            }
        }
    }
}

/// Read one open connection until it yields something, goes quiet, or is lost.
fn poll_reading(
    socket: &mut Socket,
    stale: &mut Pin<Box<Sleep>>,
    received: &mut bool,
    stale_after: Duration,
    cx: &mut Context<'_>,
) -> Step {
    loop {
        let message = match socket.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(message))) => message,
            Poll::Ready(Some(Err(source))) => {
                return Step::Lost(SportsError::Transport {
                    source: Box::new(source),
                })
            }
            Poll::Ready(None) => return Step::Lost(closed(None)),
            Poll::Pending => {
                return match stale.as_mut().poll(cx) {
                    Poll::Ready(()) => Step::Lost(SportsError::Stale { after: stale_after }),
                    Poll::Pending => Step::Pending,
                };
            }
        };
        let inbound = classify(message);
        // Anything but a close is proof of life, pings included: in a quiet
        // hour the server sends nothing else.
        if !matches!(inbound, Inbound::Closed(_)) {
            *received = true;
            stale.as_mut().reset(Instant::now() + stale_after);
        }
        match inbound {
            Inbound::Update(update) => return Step::Yield(Ok(Event::Update(update))),
            Inbound::Undecodable(error) => return Step::Yield(Err(error)),
            // Reading again is what sends the pong for a ping just read.
            Inbound::Alive => continue,
            Inbound::Closed(reason) => return Step::Lost(reason),
        }
    }
}
```

- [ ] **Step 4: Run to see it pass, and lint**

Run: `cargo test -p polyoxide-sports && cargo clippy -p polyoxide-sports --all-targets --all-features -- -D warnings`
Expected: all pass, 30 unit tests; clippy clean. If clippy reports `large_enum_variant`, a variant holding a socket or a `MatchUpdate` is not boxed. Box it, do not allow the lint.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-sports/src
git commit -F - <<'MSG'
feat(sports): supervised stream with ping-based staleness and backoff

A Stream state machine with no background task. Protocol pings count
as liveness; one Disconnected and one Reconnected per outage.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 8: Supervision tests, liveness

These tests come after the state machine because each exists to catch one specific bug. The prove steps show each test going red against that bug.

**Files:**
- Create: `polyoxide-sports/tests/supervision.rs`
- Modify: `polyoxide-sports/Cargo.toml`

- [ ] **Step 1: Write the helpers and the liveness tests**

```rust
//! The supervised tier against the scripted server.
//!
//! Staleness limits and backoff delays are tens to hundreds of milliseconds
//! so the suite runs fast. The windows are generous, to stay steady on a
//! loaded machine.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_sports::{
    fixtures,
    test_server::{Script, ScriptedServer},
    Event, SportsError, SportsWsBuilder, SupervisedSportsWs,
};
use tokio::time::{timeout, Instant};
use tokio_tungstenite::tungstenite::Message;

const STALE: Duration = Duration::from_millis(300);
const WINDOW: Duration = Duration::from_secs(3);

/// A builder pointed at `server`, with test-sized timings.
fn builder(server: &ScriptedServer) -> SportsWsBuilder {
    SportsWsBuilder::new()
        .url(server.url.clone())
        .stale_after(STALE)
        .backoff(Duration::from_millis(20), Duration::from_millis(200))
        .connect_timeout(Duration::from_secs(2))
}

/// The next item, which must arrive within the window.
async fn next_item(feed: &mut SupervisedSportsWs) -> Result<Event, SportsError> {
    timeout(WINDOW, feed.next())
        .await
        .expect("an event within the window")
        .expect("a supervised feed never ends")
}

/// Read the feed until the task is aborted, so the server sees each reconnect.
async fn drain(mut feed: SupervisedSportsWs) {
    while feed.next().await.is_some() {}
}

/// The time between consecutive accepted connections.
fn gaps(accepted: &[Instant]) -> Vec<Duration> {
    accepted.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// A short label per item, for asserting order.
fn label(item: Result<Event, SportsError>) -> String {
    match item {
        Ok(Event::Update(update)) => update.league_abbreviation.clone(),
        Ok(Event::Disconnected { .. }) => "disconnected".into(),
        Ok(Event::Reconnected) => "reconnected".into(),
        Ok(other) => format!("unexpected {other:?}"),
        Err(error) => format!("error: {error}"),
    }
}

fn text(frame: &str) -> Message {
    Message::Text(frame.into())
}

#[tokio::test]
async fn protocol_pings_alone_keep_a_quiet_connection_alive() {
    // In a quiet hour the server sends no data, only pings. If staleness
    // counted only data, a healthy connection would drop every stale period.
    let server =
        ScriptedServer::start(vec![Script::pings_only(Duration::from_millis(100))]).await;
    let mut feed = builder(&server).connect().await.unwrap();
    if let Ok(item) = timeout(STALE * 4, feed.next()).await {
        panic!("expected silence while pings flowed, got {item:?}");
    }
    assert_eq!(
        server.connection_count(),
        1,
        "the feed reconnected while pings were flowing"
    );
    assert!(!server.pongs().is_empty(), "the feed never answered a ping");
}

#[tokio::test]
async fn silence_is_stale_and_the_feed_reconnects() {
    let server = ScriptedServer::start(vec![
        Script::silent(),
        Script::pings_only(Duration::from_millis(100)),
    ])
    .await;
    let started = Instant::now();
    let mut feed = builder(&server).connect().await.unwrap();
    match next_item(&mut feed).await {
        Ok(Event::Disconnected {
            reason: SportsError::Stale { after },
        }) => assert_eq!(after, STALE),
        other => panic!("expected a stale disconnect, got {other:?}"),
    }
    assert!(started.elapsed() >= STALE, "declared stale early");
    assert!(matches!(next_item(&mut feed).await, Ok(Event::Reconnected)));
    assert_eq!(server.connection_count(), 2);
}

#[tokio::test]
async fn the_supervised_feed_answers_pings() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Ping(b"p1".to_vec().into())],
        ..Script::silent()
    }])
    .await;
    let feed = SportsWsBuilder::new()
        .url(server.url.clone())
        .connect()
        .await
        .unwrap();
    let reader = tokio::spawn(drain(feed));
    server
        .wait_for("a pong", |s| s.pongs() == [b"p1".to_vec()])
        .await;
    reader.abort();
}
```

- [ ] **Step 2: Gate the file on the feature, then run it**

In `polyoxide-sports/Cargo.toml`, add directly after the `bare` `[[test]]` entry:

```toml

[[test]]
name = "supervision"
required-features = ["test-server"]
```

Run: `cargo test -p polyoxide-sports --features test-server --test supervision`
Expected: 3 passed. The compiler may warn that `fixtures`, `gaps`, `label` and `text` are unused until Task 9. That is expected here; Task 9 uses them all.

- [ ] **Step 3: Prove the ping test catches staleness that counts only data**

In `polyoxide-sports/src/supervised.rs`, in `poll_reading`, change `if !matches!(inbound, Inbound::Closed(_)) {` to `if matches!(inbound, Inbound::Update(_)) {`. Run `cargo test -p polyoxide-sports --features test-server --test supervision protocol_pings_alone`.
Expected: FAIL with `expected silence while pings flowed, got Some(Ok(Disconnected { reason: Stale`. Revert.

- [ ] **Step 4: Prove the silence test catches a missing staleness check**

In `poll_reading`, replace the body of the `Poll::Pending =>` arm with `return Step::Pending;`. Run `cargo test -p polyoxide-sports --features test-server --test supervision silence_is_stale`.
Expected: FAIL with `an event within the window`. Revert, then run the whole file to PASS and confirm `git diff polyoxide-sports/src` is empty.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-sports/tests/supervision.rs polyoxide-sports/Cargo.toml
git commit -F - <<'MSG'
test(sports): supervised liveness on protocol pings

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 9: Supervision tests, reconnects and failures

**Files:**
- Modify: `polyoxide-sports/tests/supervision.rs` (append)

- [ ] **Step 1: Append the tests**

```rust
#[tokio::test]
async fn a_closed_connection_yields_its_updates_then_one_marker_pair() {
    let server = ScriptedServer::start(vec![
        Script::frames(&[fixtures::SOCCER, fixtures::ESPORTS]).then_close(),
        Script::frames(&[fixtures::TENNIS_EVENT_STATE]),
    ])
    .await;
    let mut feed = builder(&server).connect().await.unwrap();
    let mut labels = Vec::new();
    for _ in 0..5 {
        labels.push(label(next_item(&mut feed).await));
    }
    assert_eq!(labels, ["kor", "lol", "disconnected", "reconnected", "atp"]);
}

#[tokio::test]
async fn an_outage_yields_one_marker_pair_however_many_attempts_fail() {
    let server = ScriptedServer::start(vec![
        Script::close_at_once(),
        Script::reject(),
        Script::reject(),
        Script::reject(),
        Script::frames(&[fixtures::SOCCER]),
    ])
    .await;
    let mut feed = builder(&server)
        .stale_after(Duration::from_secs(10))
        .connect()
        .await
        .unwrap();
    let mut labels = Vec::new();
    for _ in 0..3 {
        labels.push(label(next_item(&mut feed).await));
    }
    assert_eq!(labels, ["disconnected", "reconnected", "kor"]);
    assert_eq!(
        server.connection_count(),
        5,
        "expected three refused attempts between the two good connections"
    );
}

#[tokio::test]
async fn backoff_grows_while_connections_receive_nothing() {
    // Every connection is accepted and closed at once. Nothing is received,
    // so the delay must keep doubling instead of hammering the server.
    let server = ScriptedServer::start(vec![Script::close_at_once()]).await;
    let feed = SportsWsBuilder::new()
        .url(server.url.clone())
        .backoff(Duration::from_millis(40), Duration::from_secs(5))
        .connect()
        .await
        .unwrap();
    let reader = tokio::spawn(drain(feed));
    server
        .wait_for("six connections", |s| s.connection_count() >= 6)
        .await;
    reader.abort();
    let gaps = gaps(&server.accepted_at()[..6]);
    for pair in gaps.windows(2) {
        assert!(pair[1] >= pair[0] * 3 / 2, "backoff did not grow: {gaps:?}");
    }
}

#[tokio::test]
async fn backoff_resets_after_a_connection_that_received_something() {
    let server =
        ScriptedServer::start(vec![Script::frames(&[fixtures::SOCCER]).then_close()]).await;
    let feed = SportsWsBuilder::new()
        .url(server.url.clone())
        .backoff(Duration::from_millis(40), Duration::from_secs(5))
        .connect()
        .await
        .unwrap();
    let reader = tokio::spawn(drain(feed));
    server
        .wait_for("six connections", |s| s.connection_count() >= 6)
        .await;
    reader.abort();
    let gaps = gaps(&server.accepted_at()[..6]);
    assert!(
        gaps.iter().all(|gap| *gap < Duration::from_millis(120)),
        "backoff grew although every connection delivered a frame: {gaps:?}"
    );
}

#[tokio::test]
async fn a_bad_frame_is_reported_and_the_connection_survives() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![text("not json"), text(fixtures::SOCCER)],
        ..Script::silent()
    }])
    .await;
    let mut feed = builder(&server)
        .stale_after(Duration::from_secs(10))
        .connect()
        .await
        .unwrap();
    match next_item(&mut feed).await {
        Err(SportsError::Decode { raw, .. }) => assert_eq!(raw, "not json"),
        other => panic!("expected a decode error, got {other:?}"),
    }
    assert_eq!(label(next_item(&mut feed).await), "kor");
    assert_eq!(server.connection_count(), 1, "a bad frame cost the connection");
}

#[tokio::test]
async fn dropping_the_feed_closes_the_socket() {
    let server = ScriptedServer::start(vec![Script::silent()]).await;
    let feed = builder(&server)
        .stale_after(Duration::from_secs(10))
        .connect()
        .await
        .unwrap();
    server
        .wait_for("the handshake", |s| s.handshake_count() == 1)
        .await;
    drop(feed);
    server
        .wait_for("the client to end the connection", |s| {
            s.client_ended_count() == 1
        })
        .await;
}

#[tokio::test]
async fn the_first_connection_failure_is_returned() {
    let server = ScriptedServer::start(vec![Script::reject()]).await;
    match builder(&server).connect().await {
        Err(SportsError::Connect { .. }) => {}
        Err(other) => panic!("expected a connect error, got {other}"),
        Ok(_) => panic!("connected to a server that drops every handshake"),
    }
}

#[tokio::test]
async fn a_handshake_that_never_finishes_times_out() {
    let server = ScriptedServer::start(vec![Script::stall()]).await;
    let limit = Duration::from_millis(200);
    match builder(&server).connect_timeout(limit).connect().await {
        Err(SportsError::ConnectTimeout { after }) => assert_eq!(after, limit),
        Err(other) => panic!("expected a timeout, got {other}"),
        Ok(_) => panic!("connected to a server that never answers"),
    }
}
```

- [ ] **Step 2: Run the file**

Run: `cargo test -p polyoxide-sports --features test-server --test supervision`
Expected: 11 passed, no unused warnings.

- [ ] **Step 3: Prove the outage test catches a marker per failed attempt**

In `SupervisedSportsWs::poll_next`, replace the body of the `Step::AttemptFailed(error) =>` arm with:

```rust
let delay = this.backoff.take();
this.state = State::Waiting(Box::pin(sleep(delay)));
return Poll::Ready(Some(Ok(Event::Disconnected { reason: error })));
```

Run `cargo test -p polyoxide-sports --features test-server --test supervision an_outage_yields`.
Expected: FAIL, labels `["disconnected", "disconnected", ...]`. Revert.

- [ ] **Step 4: Prove the two backoff tests catch both wrong reset rules**

In `Backoff::after_connection_ended`, delete the `if received` guard so it always resets. Run `cargo test -p polyoxide-sports --features test-server --test supervision backoff_grows`. Expected: FAIL with `backoff did not grow`. Revert.

Then make `after_connection_ended` an empty function body. Run `cargo test -p polyoxide-sports --features test-server --test supervision backoff_resets`. Expected: FAIL with `backoff grew although every connection delivered a frame`. The unit test `backoff_resets_only_after_a_connection_that_received_something` fails too. Revert, run the whole crate to PASS, and confirm `git diff polyoxide-sports/src` is empty.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-sports/tests/supervision.rs
git commit -F - <<'MSG'
test(sports): supervised reconnect, backoff and failure paths

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 10: README and crate documentation

**Files:**
- Create: `polyoxide-sports/README.md`
- Modify: `polyoxide-sports/src/lib.rs`, `polyoxide-sports/src/client.rs`, `polyoxide-sports/src/update.rs`, `polyoxide-sports/src/error.rs`

- [ ] **Step 1: Write `polyoxide-sports/README.md`**

````markdown
# polyoxide-sports

Rust client for Polymarket's live sports feed at
`wss://sports-api.polymarket.com/ws`: live scores for every match in every
league, with no credentials and no subscription.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-sports/).

## Installation

```toml
[dependencies]
polyoxide-sports = "0.36"
```

## Usage

`SportsWsBuilder` builds a feed that reconnects on its own and says when
scores may be stale:

```no_run
use futures_util::StreamExt;
use polyoxide_sports::{Event, SportsWsBuilder};

# async fn run() -> Result<(), polyoxide_sports::SportsError> {
let mut feed = SportsWsBuilder::new().connect().await?;
while let Some(event) = feed.next().await {
    match event? {
        Event::Update(update) => {
            println!("{} {} {}", update.league_abbreviation, update.score, update.period)
        }
        Event::Disconnected { reason } => eprintln!("scores are stale: {reason}"),
        Event::Reconnected => eprintln!("reconnected"),
        _ => {}
    }
}
# Ok(())
# }
```

`SportsWs` is the bare stream underneath. It yields `MatchUpdate`s and ends
when its connection does.

## What the feed sends

- **Full state, often repeated.** Each frame is a match's whole current
  state. The server re-sends unchanged state on a timer, so roughly half of
  all frames repeat the previous one for their game. `MatchUpdate`
  implements `PartialEq`: compare against the last frame per `GameKey` to
  keep only changes.
- **The ended frame is sent once.** A feed that is disconnected when a
  match ends never sees it. After `Event::Reconnected`, look up games that
  may have ended through gamma's events list, which filters on `game_id`.
  Cricket uses a string id that gamma does not accept.
- **Two kinds of id.** Most sports send a numeric `gameId`; cricket sends a
  string `metadataGameId` instead. `MatchUpdate::key` returns a `GameKey`
  that covers both.
- **Protocol pings every 15 seconds.** The transport answers them. When
  nothing is live anywhere they are the only sign of life, so the supervised
  feed counts them toward its 45-second staleness limit.

Polymarket's published AsyncAPI document for this host describes a payload
and a text keep-alive that the server does not use. This crate is modelled
on captured frames instead.
````

- [ ] **Step 2: Replace `polyoxide-sports/src/lib.rs`**

```rust
//! # polyoxide-sports
//!
//! Rust client for Polymarket's live sports feed at
//! `wss://sports-api.polymarket.com/ws`.
//!
//! The feed needs no credentials and takes no subscription: a connection
//! receives every live match in every league. Each frame is a
//! [`MatchUpdate`] carrying one match's full current state, not a change.
//!
//! Two tiers:
//!
//! - [`SportsWs`] is a bare stream that ends when its connection does.
//! - [`SportsWsBuilder`] builds a [`SupervisedSportsWs`], which reconnects
//!   for as long as it is held and yields [`Event::Disconnected`] and
//!   [`Event::Reconnected`] around every outage.
//!
//! The AsyncAPI document Polymarket publishes for this host does not match
//! the wire. Everything here is modelled on captured frames, and the
//! differences are recorded in `docs/specs/sports/OBSERVED.md` in the
//! repository.

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod client;
pub mod error;
pub mod supervised;
pub mod update;

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;

#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;

pub use client::{SportsWs, SPORTS_WS_URL};
pub use error::SportsError;
pub use supervised::{Event, SportsWsBuilder, SupervisedSportsWs};
pub use update::{GameKey, MatchUpdate};
```

- [ ] **Step 3: Turn the forward references into links**

Now that every item exists, replace three plain-backtick references:

- In `client.rs`, in the `SportsWs` doc comment: `/// For a feed that reconnects on its own, use `SportsWsBuilder`.` becomes `/// For a feed that reconnects on its own, use [`SportsWsBuilder`](crate::SportsWsBuilder).`
- In `update.rs`, in the `MatchUpdate` doc comment: `/// disconnected at that moment never sees it; the supervised stream's` and the next line `/// `Event::Reconnected` says when that may have happened.` become `/// disconnected at that moment never sees it;` and `/// [`Event::Reconnected`](crate::Event::Reconnected) says when that may have happened.`
- In `error.rs`, in the `SportsError` doc comment: `/// as an `Err`. Every other variant arrives inside `Event::Disconnected`,` becomes `/// as an `Err`. Every other variant arrives inside [`Event::Disconnected`](crate::Event::Disconnected),`

- [ ] **Step 4: Run the doctests and the doc build**

Run:

```bash
cargo test -p polyoxide-sports --doc --all-features
RUSTDOCFLAGS="-D warnings" cargo doc -p polyoxide-sports --no-deps --all-features
```

Expected: 3 doctests pass (the README's Rust block, `SportsWs` and `SportsWsBuilder`; the README's `toml` block is not compiled), and the doc build has no warnings.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-sports
git commit -F - <<'MSG'
docs(sports): README and crate documentation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 11: The live suite

**Files:**
- Create: `polyoxide-sports/tests/live_api.rs`

- [ ] **Step 1: Write the live tests**

```rust
//! Live tests against wss://sports-api.polymarket.com/ws. Ignored by default:
//!
//! ```text
//! cargo test -p polyoxide-sports --test live_api -- --ignored --nocapture
//! ```
//!
//! The feed carries only matches that are live somewhere. A test that waits
//! for a frame says so when it times out, in the words the nightly
//! classifier treats as environmental.
//!
//! `nightly-schema.yml` excludes this host, because the published AsyncAPI
//! document does not match the wire. So
//! `live_frames_round_trip_and_carry_no_unmodelled_keys` is this host's drift
//! detector: when upstream adds a field, it fails and names the key.

use std::{collections::BTreeMap, time::Duration};

use futures_util::StreamExt;
use polyoxide_sports::{Event, MatchUpdate, SportsWs, SportsWsBuilder, SPORTS_WS_URL};
use serde_json::Value;
use tokio::time::{timeout, timeout_at, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const RECV_WINDOW: Duration = Duration::from_secs(45);
const QUIET: &str = "if no matches are live anywhere this can legitimately time out, \
                     so re-run before concluding a defect";

/// The raw socket tests call `connect_async` directly, so they install the
/// provider the crate would. See `ensure_crypto_provider` in src/client.rs.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[tokio::test]
#[ignore]
async fn live_bare_feed_yields_a_parsed_frame() {
    let mut feed = SportsWs::connect().await.expect("connect to the sports feed");
    let update = timeout(RECV_WINDOW, feed.next())
        .await
        .unwrap_or_else(|_| panic!("no frame within {RECV_WINDOW:?}; {QUIET}"))
        .expect("the stream ended instead of yielding a frame")
        .expect("the frame parses");
    assert!(!update.league_abbreviation.is_empty(), "{update:?}");
    assert!(update.key().is_some(), "the frame identifies no match: {update:?}");
    println!("first frame: {update:?}");
}

/// Upstream documents a text "ping"/"pong" exchange that a client must
/// answer within 10 seconds. The server actually sends protocol pings that
/// the transport answers. If upstream were right, a client that never sends
/// a text "pong" would be dropped well inside this window.
#[tokio::test]
#[ignore]
async fn live_bare_connection_survives_the_keepalive_interval() {
    let mut feed = SportsWs::connect().await.expect("connect to the sports feed");
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut frames = 0usize;
    loop {
        match timeout_at(deadline, feed.next()).await {
            Err(_) => break,
            Ok(Some(Ok(_))) => frames += 1,
            Ok(Some(Err(e))) => panic!("the stream failed after {frames} frames: {e}"),
            Ok(None) => panic!(
                "the server closed the connection after {frames} frames; the keep-alive \
                 is not being answered"
            ),
        }
    }
    println!("survived 40 s with {frames} frames");
}

/// The 45-second staleness default rests on this cadence.
#[tokio::test]
#[ignore]
async fn live_server_sends_protocol_pings_every_15_seconds() {
    install_crypto_provider();
    let (mut socket, _) = connect_async(SPORTS_WS_URL).await.expect("connect");
    let started = Instant::now();
    let deadline = started + Duration::from_secs(40);
    let mut pings = Vec::new();
    loop {
        match timeout_at(deadline, socket.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Message::Ping(_)))) => pings.push(started.elapsed()),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => panic!("the socket failed: {e}"),
            Ok(None) => panic!("the server closed the socket"),
        }
    }
    assert!(
        pings.len() >= 2,
        "expected at least two protocol pings in 40 s, saw {pings:?}; the staleness limit depends on them"
    );
    let gaps: Vec<Duration> = pings.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        gaps.iter().all(|gap| *gap < Duration::from_secs(20)),
        "ping gaps {gaps:?} exceed 20 s; revisit the 45 s staleness default"
    );
}

/// Held past the 45-second staleness limit with default settings, the
/// supervised feed must not disconnect.
#[tokio::test]
#[ignore]
async fn live_supervised_feed_holds_past_the_stale_limit() {
    let mut feed = SportsWsBuilder::new()
        .connect()
        .await
        .expect("connect to the sports feed");
    let deadline = Instant::now() + Duration::from_secs(50);
    let mut updates = 0usize;
    loop {
        match timeout_at(deadline, feed.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Event::Update(_)))) => updates += 1,
            Ok(Some(Ok(Event::Disconnected { reason }))) => {
                panic!("disconnected after {updates} updates: {reason}")
            }
            Ok(Some(Ok(other))) => panic!("unexpected event {other:?}"),
            Ok(Some(Err(e))) => panic!("a live frame did not parse: {e}"),
            Ok(None) => panic!("a supervised feed never ends"),
        }
    }
    println!("held 50 s with {updates} updates");
}

#[tokio::test]
#[ignore]
async fn live_frames_round_trip_and_carry_no_unmodelled_keys() {
    install_crypto_provider();
    let (mut socket, _) = connect_async(SPORTS_WS_URL).await.expect("connect");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut checked = 0usize;
    // Each unmodelled key, with a league it was seen on.
    let mut unmodelled: BTreeMap<String, String> = BTreeMap::new();
    while checked < 100 {
        let message = match timeout_at(deadline, socket.next()).await {
            Err(_) => break,
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(e))) => panic!("the socket failed: {e}"),
            Ok(None) => panic!("the server closed the socket"),
        };
        let Message::Text(text) = message else {
            continue;
        };
        let update = MatchUpdate::from_json(&text)
            .unwrap_or_else(|e| panic!("a live frame did not parse: {e}\n{}", text.as_str()));
        let original: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&update).unwrap(),
            original,
            "a live frame changed on a round trip"
        );
        for key in update.extra.keys() {
            unmodelled
                .entry(key.clone())
                .or_insert_with(|| update.league_abbreviation.clone());
        }
        checked += 1;
    }
    assert!(checked > 0, "no frames within 60 s; {QUIET}");
    assert!(
        unmodelled.is_empty(),
        "the feed sends keys MatchUpdate does not model (key: league seen on): {unmodelled:?}. \
         Model them in MatchUpdate, refresh fixtures with scripts/capture_sports_fixtures.py, \
         and record them in docs/specs/sports/OBSERVED.md"
    );
    println!("checked {checked} frames");
}
```

- [ ] **Step 2: Confirm it compiles and is skipped by default**

Run: `cargo test -p polyoxide-sports --test live_api`
Expected: `0 passed; 0 failed; 5 ignored`.

- [ ] **Step 3: Run it live once**

Run: `cargo test -p polyoxide-sports --test live_api -- --ignored --nocapture`
Expected: 5 passed, taking about a minute. A timeout whose message says "legitimately time out" means nothing was live; rerun later. If the wire-agreement test names an unmodelled key, stop and report it to the user with the frame. Do not add fields without a captured fixture.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-sports/tests/live_api.rs
git commit -F - <<'MSG'
test(sports): live suite, including wire agreement as drift detector

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 12: Fixture capture script

**Files:**
- Create: `scripts/capture_sports_fixtures.py`

- [ ] **Step 1: Write the script**

```python
#!/usr/bin/env python3
"""Capture live frames from Polymarket's sports feed as test fixtures.

Usage:
    uv run --with websockets --with certifi python3 scripts/capture_sports_fixtures.py OUT_DIR [SECONDS]

Records wss://sports-api.polymarket.com/ws for SECONDS (default 300). Keeps
the first frame of each distinct top-level key-set, and of each
`eventState.type`, byte for byte as `<league>-<n>.json`. Writes
PROVENANCE.md with the date, the frame count per league, the protocol ping
times and the longest gap between data frames.

Write to a scratch directory, not the fixtures directory. Copy the frames
worth keeping into polyoxide-sports/tests/fixtures/ and list each one in
polyoxide-sports/src/fixtures.rs; `every_fixture_file_is_listed` fails
otherwise. Run it in a busy window, such as a weekend afternoon UTC, to
see the most sports.
"""

import asyncio
import json
import re
import ssl
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

import certifi
from websockets.asyncio.client import ClientConnection, connect
from websockets.frames import Opcode

URL = "wss://sports-api.polymarket.com/ws"


class PingRecorder(ClientConnection):
    """Notes when each protocol ping arrives.

    The handshake response passes through `process_event` too, and it has
    no opcode, so the check must not assume one.
    """

    started = 0.0
    pings: list[float] = []

    def process_event(self, event):
        if getattr(event, "opcode", None) is Opcode.PING:
            PingRecorder.pings.append(round(time.monotonic() - PingRecorder.started, 2))
        return super().process_event(event)


async def record(seconds: int) -> list[tuple[float, str]]:
    """Every text frame received in `seconds`, with its arrival time."""
    ctx = ssl.create_default_context(cafile=certifi.where())
    frames = []
    PingRecorder.started = time.monotonic()
    async with connect(URL, ssl=ctx, ping_interval=None, create_connection=PingRecorder) as ws:
        end = PingRecorder.started + seconds
        while (remaining := end - time.monotonic()) > 0:
            try:
                message = await asyncio.wait_for(ws.recv(), remaining)
            except asyncio.TimeoutError:
                break
            if isinstance(message, str):
                frames.append((round(time.monotonic() - PingRecorder.started, 2), message))
    return frames


def shape_marks(raw: str) -> list[tuple[str, object]]:
    """What makes a frame's shape distinct: its key-set and eventState type."""
    try:
        frame = json.loads(raw)
    except json.JSONDecodeError:
        return [("unparsed", raw[:40])]
    if not isinstance(frame, dict):
        return [("not-an-object", type(frame).__name__)]
    marks: list[tuple[str, object]] = [("keys", tuple(sorted(frame)))]
    state = frame.get("eventState")
    if isinstance(state, dict):
        marks.append(("eventState", state.get("type")))
    return marks


def league_of(raw: str) -> str:
    try:
        return str(json.loads(raw).get("leagueAbbreviation", "unknown"))
    except (json.JSONDecodeError, AttributeError):
        return "unparsed"


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", text.lower()).strip("_") or "unknown"


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    seconds = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    out.mkdir(parents=True, exist_ok=True)

    captured_at = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    frames = asyncio.run(record(seconds))

    seen: set = set()
    kept = []
    for _, raw in frames:
        marks = shape_marks(raw)
        if any(mark not in seen for mark in marks):
            seen.update(marks)
            name = f"{slug(league_of(raw))}-{len(kept)}.json"
            # No trailing newline: the file is the frame, byte for byte.
            (out / name).write_text(raw, encoding="utf-8")
            kept.append((name, marks))

    times = [t for t, _ in frames]
    longest_gap = max((b - a for a, b in zip(times, times[1:])), default=0.0)
    leagues = Counter(league_of(raw) for _, raw in frames)
    lines = [
        "# Sports capture provenance",
        "",
        f"- Captured: {captured_at}, for {seconds} s, from `{URL}`",
        f"- Frames: {len(frames)}",
        f"- Protocol pings at (s): {PingRecorder.pings}",
        f"- Longest gap between data frames: {longest_gap:.1f} s",
        "",
        "| League | Frames |",
        "|---|---|",
        *[f"| {league} | {count} |" for league, count in leagues.most_common()],
        "",
        "| File | Shape |",
        "|---|---|",
        *[f"| `{name}` | {marks} |" for name, marks in kept],
    ]
    (out / "PROVENANCE.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(frames)} frames, {len(kept)} distinct shapes, written to {out}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it for a minute into a scratch directory**

```bash
OUT=$(mktemp -d)
uv run -q --with websockets --with certifi python3 scripts/capture_sports_fixtures.py "$OUT" 60
cat "$OUT/PROVENANCE.md"; ls "$OUT"
```

Expected: a line such as `48 frames, 3 distinct shapes, written to /tmp/...`. `PROVENANCE.md` lists about four ping times 15 s apart, and each `.json` file is one line with no trailing newline (`tail -c1 "$OUT"/*.json | od -c` shows no `\n`). If the capture shows a shape no fixture covers, report it to the user. Do not add it in this task.

- [ ] **Step 3: Commit**

```bash
chmod +x scripts/capture_sports_fixtures.py
git add scripts/capture_sports_fixtures.py
git commit -F - <<'MSG'
feat(scripts): capture sports feed fixtures with ping timings

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 13: `polyoxide ws sports`

**Files:**
- Create: `polyoxide-cli/src/commands/ws/sports.rs`
- Modify: `polyoxide-cli/src/commands/ws/mod.rs`, `polyoxide-cli/src/main.rs` (tests), `polyoxide-cli/Cargo.toml`

- [ ] **Step 1: Add the dependency**

In `polyoxide-cli/Cargo.toml`, add after `polyoxide-rtds = { workspace = true }` in `[dependencies]`:

```toml
polyoxide-sports = { workspace = true }
```

and in `[dev-dependencies]`:

```toml
# The captured frames, for the `ws sports` tests.
polyoxide-sports = { workspace = true, features = ["test-server"] }
```

- [ ] **Step 2: Write `polyoxide-cli/src/commands/ws/sports.rs`**

```rust
//! `polyoxide ws sports`: stream live match updates from the sports feed.

use std::{collections::HashMap, io::Write, time::Duration};

use clap::Args;
use color_eyre::eyre::Result;
use futures_util::{Stream, StreamExt};
use polyoxide_sports::{Event, GameKey, MatchUpdate, SportsError, SportsWsBuilder};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};

/// How each update is printed.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable, one line per update.
    #[default]
    Pretty,
    /// The update as compact JSON, one object per line.
    Json,
}

#[derive(Args, Debug)]
pub struct SportsArgs {
    /// Leagues to keep, comma-separated, e.g. `atp,wta`. Matching ignores
    /// case. Omit to keep every league.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub league: Vec<String>,

    /// Games to keep, comma-separated. Takes numeric ids and cricket's `id…`
    /// ids alike. Omit to keep every game.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub game: Vec<String>,

    /// Skip a frame identical to the last one printed for its game. The
    /// server re-sends unchanged state on a timer, so about half of all
    /// frames are repeats.
    #[arg(long)]
    pub changes_only: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,

    /// Exit after printing N updates
    #[arg(short = 'n', long)]
    pub count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
}

/// Connect to the production feed and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: SportsArgs) -> Result<()> {
    eprintln!("Connecting to the sports feed...");
    let feed = SportsWsBuilder::new().connect().await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    run_with(args, feed, &mut std::io::stdout(), &mut std::io::stderr()).await
}

/// Filter and print events from any stream until `-n` or `-t` is reached or
/// the stream ends.
///
/// Takes the stream rather than connecting, so tests can drive every flag
/// with a scripted list of events. Updates go to `out`; connection markers
/// and skipped frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: SportsArgs,
    mut events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Event, SportsError>> + Unpin,
{
    let deadline = args.timeout.map(|t| tokio::time::Instant::now() + t);
    let mut filter = Filter::new(&args);
    let mut printed: u64 = 0;
    loop {
        if args.count.is_some_and(|n| printed >= n) {
            break;
        }
        let next = match deadline {
            Some(deadline) => match tokio::time::timeout_at(deadline, events.next()).await {
                Ok(next) => next,
                Err(_) => {
                    writeln!(err, "Timeout reached")?;
                    break;
                }
            },
            None => events.next().await,
        };
        match next {
            Some(Ok(Event::Update(update))) => {
                if filter.admits(&update) {
                    print_update(&update, args.format, out)?;
                    printed += 1;
                }
            }
            Some(Ok(Event::Disconnected { reason })) => writeln!(
                err,
                "# disconnected: {reason}. Scores are stale until reconnected."
            )?,
            Some(Ok(Event::Reconnected)) => writeln!(
                err,
                "# reconnected. Games that ended during the gap were not re-sent."
            )?,
            // `Event` is #[non_exhaustive]; a future variant is not a fault.
            Some(Ok(_)) => {}
            Some(Err(error)) => writeln!(err, "# skipped a frame: {error}")?,
            None => {
                writeln!(err, "The feed ended")?;
                break;
            }
        }
    }
    Ok(())
}

/// `--league`, `--game` and `--changes-only` as one decision per update.
struct Filter {
    leagues: Vec<String>,
    games: Vec<String>,
    changes_only: bool,
    last: HashMap<GameKey, MatchUpdate>,
}

impl Filter {
    fn new(args: &SportsArgs) -> Self {
        Self {
            leagues: args.league.iter().map(|l| l.to_lowercase()).collect(),
            games: args.game.clone(),
            changes_only: args.changes_only,
            last: HashMap::new(),
        }
    }

    fn admits(&mut self, update: &MatchUpdate) -> bool {
        if !self.leagues.is_empty()
            && !self
                .leagues
                .contains(&update.league_abbreviation.to_lowercase())
        {
            return false;
        }
        let key = update.key();
        if !self.games.is_empty() {
            match &key {
                Some(key) if self.games.contains(&key.to_string()) => {}
                _ => return false,
            }
        }
        if self.changes_only {
            if let Some(key) = key {
                if self.last.get(&key) == Some(update) {
                    return false;
                }
                self.last.insert(key, update.clone());
            }
        }
        true
    }
}

fn print_update(update: &MatchUpdate, format: OutputFormat, out: &mut dyn Write) -> Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => {
            let id = update
                .key()
                .map(|key| key.to_string())
                .unwrap_or_else(|| "-".into());
            let teams = match (&update.home_team, &update.away_team) {
                (Some(home), Some(away)) => format!("{home} v {away}"),
                _ => "-".into(),
            };
            let state = if update.ended {
                "ended"
            } else if update.live {
                "live"
            } else {
                "not started"
            };
            writeln!(
                out,
                "{:<16} {:>20}  {:<44} {:>18} {:>6}  {}",
                update.league_abbreviation, id, teams, update.score, update.period, state
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: SportsArgs,
    }

    fn parse(argv: &[&str]) -> SportsArgs {
        Wrapper::try_parse_from(argv).unwrap().args
    }

    #[test]
    fn league_and_game_split_on_commas_and_are_trimmed() {
        let args = parse(&[
            "test",
            "--league",
            "atp, wta challenger",
            "--game",
            "1712005,id2704098174740616",
        ]);
        assert_eq!(args.league, ["atp", "wta challenger"]);
        assert_eq!(args.game, ["1712005", "id2704098174740616"]);
    }

    #[test]
    fn repeated_flags_accumulate() {
        let args = parse(&["test", "--league", "atp", "--league", "wta"]);
        assert_eq!(args.league, ["atp", "wta"]);
    }

    #[test]
    fn defaults_keep_everything_forever() {
        let args = parse(&["test"]);
        assert!(args.league.is_empty() && args.game.is_empty());
        assert!(!args.changes_only);
        assert_eq!(args.format, OutputFormat::Pretty);
        assert_eq!(args.count, None);
        assert_eq!(args.timeout, None);
    }

    #[test]
    fn count_and_timeout_parse() {
        let args = parse(&["test", "-n", "3", "-t", "5m", "--format", "json"]);
        assert_eq!(args.count, Some(3));
        assert_eq!(args.timeout, Some(Duration::from_secs(300)));
        assert_eq!(args.format, OutputFormat::Json);
    }
}
```

- [ ] **Step 3: Wire the subcommand**

In `polyoxide-cli/src/commands/ws/mod.rs`, add `pub mod sports;` after `mod prices;`, add this variant to `WsCommand` after `Prices`:

```rust
    /// Stream live match scores from the sports feed
    Sports {
        #[command(flatten)]
        args: sports::SportsArgs,
    },
```

and this arm to `WsCommand::run` after the `Prices` arm:

```rust
            Self::Sports { args } => sports::run(args).await,
```

In `polyoxide-cli/src/main.rs`, add to the test module after `ws_market_parses_with_asset_id`:

```rust
    #[test]
    fn ws_sports_parses_with_no_arguments() {
        let cli = try_parse(&["polyoxide", "ws", "sports"]).unwrap();
        assert!(matches!(
            cli.command,
            super::Commands::Ws {
                command: polyoxide_cli::commands::WsCommand::Sports { .. }
            }
        ));
    }
```

- [ ] **Step 4: Run the tests and lint**

Run: `cargo test -p polyoxide-cli sports && cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings`
Expected: 5 tests pass (4 in `ws::sports::tests`, 1 in `main`); clippy clean.

- [ ] **Step 5: Prove the comma test catches a list flag without a delimiter**

Remove `value_delimiter = ',', ` from the `league` field. Run `cargo test -p polyoxide-cli league_and_game_split`.
Expected: FAIL, `league` is `["atp, wta challenger"]`. Revert.

- [ ] **Step 6: Try it against the live feed**

Run: `cargo run -q -p polyoxide-cli -- ws sports -n 3 --changes-only`
Expected: three pretty lines, or nothing for up to a minute when no match is live. Ctrl+C exits.

- [ ] **Step 7: Commit**

```bash
git add polyoxide-cli Cargo.lock
git commit -F - <<'MSG'
feat(cli): ws sports streams live match scores

Comma-separated --league and --game filters, --changes-only to drop
the server's repeated frames, and connection markers on stderr.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 14: `ws sports` behaviour tests and live test

**Files:**
- Create: `polyoxide-cli/tests/ws_sports.rs`
- Modify: `polyoxide-cli/tests/live_api.rs` (append)

- [ ] **Step 1: Write the behaviour tests**

```rust
//! `polyoxide ws sports` over scripted event streams.
//!
//! Each test parses real arguments and runs the command over events built
//! from captured frames, so a flag that parses but never reaches the filter
//! fails here.

use std::time::Duration;

use clap::Parser;
use futures_util::stream;
use polyoxide_cli::commands::ws::sports::{run_with, SportsArgs};
use polyoxide_sports::{fixtures, Event, MatchUpdate, SportsError};
use serde_json::Value;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    args: SportsArgs,
}

type Item = Result<Event, SportsError>;

fn update(frame: &str) -> Item {
    Ok(Event::Update(Box::new(MatchUpdate::from_json(frame).unwrap())))
}

fn with_score(frame: &str, score: &str) -> Item {
    let mut value: Value = serde_json::from_str(frame).unwrap();
    value["score"] = Value::from(score);
    Ok(Event::Update(Box::new(serde_json::from_value(value).unwrap())))
}

/// What one run wrote.
struct Run {
    out: String,
    err: String,
}

impl Run {
    /// Each stdout line, parsed as JSON.
    fn rows(&self) -> Vec<Value> {
        self.out
            .lines()
            .map(|line| {
                serde_json::from_str(line)
                    .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line}"))
            })
            .collect()
    }

    /// One string field from every row.
    fn field(&self, name: &str) -> Vec<String> {
        self.rows()
            .iter()
            .map(|row| row[name].as_str().unwrap().to_owned())
            .collect()
    }
}

async fn run_argv(argv: &[&str], events: Vec<Item>) -> Run {
    let cli = Cli::try_parse_from(argv).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    run_with(cli.args, stream::iter(events), &mut out, &mut err)
        .await
        .unwrap();
    Run {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// Run with JSON output plus `flags`.
async fn run(flags: &[&str], events: Vec<Item>) -> Run {
    let mut argv = vec!["sports", "--format", "json"];
    argv.extend_from_slice(flags);
    run_argv(&argv, events).await
}

#[tokio::test]
async fn every_update_is_printed_without_filters() {
    let run = run(&[], vec![update(fixtures::SOCCER), update(fixtures::CRICKET)]).await;
    assert_eq!(run.field("leagueAbbreviation"), ["kor", "cricket"]);
}

#[tokio::test]
async fn the_league_filter_splits_on_commas_and_ignores_case() {
    let run = run(
        &["--league", "ATP,Wta Challenger"],
        vec![
            update(fixtures::TENNIS_EVENT_STATE),
            update(fixtures::LEAGUE_WITH_SPACE),
            update(fixtures::SOCCER),
        ],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["atp", "wta challenger"]);
}

#[tokio::test]
async fn the_game_filter_matches_both_kinds_of_id() {
    let run = run(
        &["--game", "90106111,id2703680373085574"],
        vec![
            update(fixtures::SOCCER),
            update(fixtures::ESPORTS),
            update(fixtures::CRICKET),
        ],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["kor", "cricket"]);
}

#[tokio::test]
async fn changes_only_drops_repeats_and_keeps_changes() {
    let events = || {
        vec![
            update(fixtures::SOCCER),
            update(fixtures::SOCCER),
            with_score(fixtures::SOCCER, "3-1"),
            with_score(fixtures::SOCCER, "3-1"),
            update(fixtures::ESPORTS),
        ]
    };
    let changes = run(&["--changes-only"], events()).await;
    assert_eq!(changes.field("score"), ["2-1", "3-1", "000-000|0-0|Bo1"]);
    let everything = run(&[], events()).await;
    assert_eq!(
        everything.rows().len(),
        5,
        "without --changes-only every frame is printed"
    );
}

#[tokio::test]
async fn count_stops_after_n_printed_updates_not_n_received() {
    let run = run(
        &["-n", "2", "--league", "cricket"],
        vec![
            update(fixtures::SOCCER),
            update(fixtures::CRICKET),
            update(fixtures::ESPORTS),
            update(fixtures::CRICKET_FINISHED),
            update(fixtures::CRICKET),
        ],
    )
    .await;
    assert_eq!(run.field("score"), ["21-178", "116-38"]);
}

#[tokio::test]
async fn markers_go_to_stderr_and_stdout_stays_jsonl() {
    let run = run(
        &[],
        vec![
            update(fixtures::SOCCER),
            Ok(Event::Disconnected {
                reason: SportsError::Stale {
                    after: Duration::from_secs(45),
                },
            }),
            Ok(Event::Reconnected),
            update(fixtures::ESPORTS),
        ],
    )
    .await;
    assert_eq!(run.rows().len(), 2);
    assert!(
        run.err.contains("disconnected") && run.err.contains("pings included"),
        "{}",
        run.err
    );
    assert!(run.err.contains("reconnected"), "{}", run.err);
}

#[tokio::test]
async fn a_bad_frame_is_reported_and_streaming_continues() {
    let source = serde_json::from_str::<Value>("not json").unwrap_err();
    let run = run(
        &[],
        vec![
            Err(SportsError::Decode {
                raw: "not json".into(),
                source,
            }),
            update(fixtures::SOCCER),
        ],
    )
    .await;
    assert_eq!(run.field("leagueAbbreviation"), ["kor"]);
    assert!(run.err.contains("skipped a frame"), "{}", run.err);
}

#[tokio::test]
async fn the_timeout_ends_a_quiet_feed() {
    let cli = Cli::try_parse_from(["sports", "--timeout", "100ms"]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    run_with(cli.args, stream::pending::<Item>(), &mut out, &mut err)
        .await
        .unwrap();
    assert!(out.is_empty());
    assert!(String::from_utf8(err).unwrap().contains("Timeout reached"));
}

#[tokio::test]
async fn pretty_output_shows_teams_score_and_state() {
    let run = run_argv(
        &["sports"],
        vec![update(fixtures::SOCCER), update(fixtures::CRICKET_FINISHED)],
    )
    .await;
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", run.out);
    for expected in [
        "kor",
        "90106111",
        "Gimcheon Sangmu FC v Daejeon Hana Citizen FC",
        "2-1",
        "2H",
        "live",
    ] {
        assert!(lines[0].contains(expected), "{expected} missing from {:?}", lines[0]);
    }
    for expected in ["cricket", "id2703438269077680", "116-38", "FT", "ended"] {
        assert!(lines[1].contains(expected), "{expected} missing from {:?}", lines[1]);
    }
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p polyoxide-cli --test ws_sports`
Expected: 9 passed.

- [ ] **Step 3: Prove two filters are really exercised**

In `Filter::new` in `polyoxide-cli/src/commands/ws/sports.rs`, change `args.league.iter().map(|l| l.to_lowercase()).collect()` to `args.league.clone()`. Run `cargo test -p polyoxide-cli --test ws_sports the_league_filter`. Expected: FAIL, leagues `[]`. Revert.

In `Filter::admits`, delete the line `self.last.insert(key, update.clone());`. Run `cargo test -p polyoxide-cli --test ws_sports changes_only`. Expected: FAIL, five scores printed. Revert, then run the file to PASS.

- [ ] **Step 4: Append the live test to `polyoxide-cli/tests/live_api.rs`**

```rust
// ── ws sports ────────────────────────────────────────────────────────

mod ws_sports {
    use clap::Parser;
    use polyoxide_cli::commands::ws::sports::{run_with, SportsArgs};
    use polyoxide_sports::SportsWsBuilder;
    use serde_json::Value;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: SportsArgs,
    }

    #[tokio::test]
    #[ignore = "hits the real Polymarket API"]
    async fn live_ws_sports_prints_one_json_line() {
        let cli = Cli::try_parse_from(["sports", "-n", "1", "--format", "json", "--timeout", "60s"])
            .unwrap();
        let feed = SportsWsBuilder::new()
            .connect()
            .await
            .expect("connect to the sports feed");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        run_with(cli.args, feed, &mut out, &mut err).await.unwrap();
        let out = String::from_utf8(out).unwrap();
        let Some(line) = out.lines().next() else {
            panic!(
                "no update within 60 s; if no matches are live anywhere this can legitimately \
                 time out, so re-run before concluding a defect. stderr: {}",
                String::from_utf8_lossy(&err)
            );
        };
        let update: Value = serde_json::from_str(line).unwrap();
        assert!(update["leagueAbbreviation"].is_string(), "{update}");
    }
}
```

- [ ] **Step 5: Run the live test once**

Run: `cargo test -p polyoxide-cli --test live_api live_ws_sports -- --ignored --nocapture`
Expected: 1 passed.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-cli/tests
git commit -F - <<'MSG'
test(cli): ws sports filters, output and live run

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 15: The unified crate's `sports` feature

**Files:**
- Modify: `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`

- [ ] **Step 1: Add the feature and dependency**

In `polyoxide/Cargo.toml` `[features]`, add after `perps-ws = [...]`:

```toml
sports = ["dep:polyoxide-sports"]
```

and change `full` to:

```toml
full = ["clob", "gamma", "data", "ws", "rtds", "perps", "perps-ws", "sports"]
```

In `[dependencies]`, add after `polyoxide-perps = ...`:

```toml
polyoxide-sports = { workspace = true, optional = true }
```

- [ ] **Step 2: Re-export it**

In `polyoxide/src/lib.rs`:

- Replace the first two doc lines with:

  ```rust
  //! Unified Rust client for Polymarket APIs, combining CLOB (trading), Gamma (market data)
  //! and Data APIs, with RTDS price streams, Perps market data and live sports scores behind
  //! feature flags.
  ```

- Replace the first `## Features` bullet (three lines, starting `//! - Unified access to CLOB, Gamma, and Data APIs, plus RTDS price streams and`) with:

  ```rust
  //! - Unified access to CLOB, Gamma, and Data APIs, plus RTDS price streams, Perps
  //!   public market data and live sports scores behind the `rtds`, `perps` and
  //!   `sports` features, and the Perps WebSocket channels behind `perps-ws`
  ```

- After `pub use polyoxide_rtds;` and its `#[cfg(feature = "rtds")]`, add:

  ```rust
  #[cfg(feature = "sports")]
  pub use polyoxide_sports;
  ```

- In the `prelude` module, after the `#[cfg(feature = "rtds")] pub use polyoxide_rtds::{...};` block, add:

  ```rust
      #[cfg(feature = "sports")]
      pub use polyoxide_sports::{
          Event as SportsEvent, GameKey, MatchUpdate, SportsError, SportsWs, SportsWsBuilder,
          SupervisedSportsWs,
      };
  ```

- [ ] **Step 3: Document it in `polyoxide/README.md`**

- Line 3: change `RTDS (price streams) and Perps (perpetual futures market data) crates` to `RTDS (price streams), Perps (perpetual futures market data) and Sports (live scores) crates`.
- Line 9: change `` `rtds`, `perps` and `perps-ws` are opt-in: `` to `` `rtds`, `perps`, `perps-ws` and `sports` are opt-in: ``.
- In the feature table, add after the `perps-ws` row:

  ```markdown
  | `sports` | no | Live sports scores via `polyoxide-sports` |
  ```

- In the `full` row, change `` + `perps` + `perps-ws` |`` to `` + `perps` + `perps-ws` + `sports` |``.

- [ ] **Step 4: Build every combination that matters**

Run:

```bash
cargo build -p polyoxide --no-default-features --features sports
cargo build -p polyoxide --all-features
cargo test -p polyoxide --all-features --doc
```

Expected: all succeed. The doctests include both READMEs.

- [ ] **Step 5: Commit**

```bash
git add polyoxide Cargo.lock
git commit -F - <<'MSG'
feat(polyoxide): sports feature re-exporting polyoxide-sports

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 16: Remove the sports channel from clob (breaking)

**Files:**
- Delete: `polyoxide-clob/src/ws/sports.rs`
- Modify: `polyoxide-clob/src/ws/mod.rs`, `polyoxide-clob/src/ws/subscription.rs`, `polyoxide-clob/src/ws/client.rs`, `polyoxide-clob/README.md`, `polyoxide-clob/tests/live_ws.rs`

- [ ] **Step 1: Delete the module**

```bash
git rm polyoxide-clob/src/ws/sports.rs
```

- [ ] **Step 2: Edit `polyoxide-clob/src/ws/mod.rs`**

- Replace the three-line bullet starting `//! - **Sports Channel**: Public channel for live match updates, on its own host and` with:

  ```rust
  //! The live sports feed is not a CLOB channel. It is served by another host and
  //! lives in the `polyoxide-sports` crate.
  ```

- Delete the line `mod sports;`.
- Delete the line `pub use sports::{SportsMessage, SportsUpdateMessage};`.
- In the `pub use subscription::{...}` list, change `WS_MARKET_URL, WS_SPORTS_URL, WS_USER_URL,` to `WS_MARKET_URL, WS_USER_URL,`.
- In `pub enum Channel`, delete:

  ```rust
      /// Sports channel message
      Sports(SportsMessage),
  ```

- [ ] **Step 3: Edit `polyoxide-clob/src/ws/subscription.rs`**

- Delete the `WS_SPORTS_URL` constant with its four doc lines and the blank line after it.
- In `pub enum ChannelType`, delete:

  ```rust
      /// Sports channel for live game state updates
      ///
      /// Unlike the other two, this variant is never sent on the wire — the
      /// sports channel takes no subscription payload. It exists so a connected
      /// [`WebSocket`](crate::ws::WebSocket) can report which channel it is on.
      Sports,
  ```

- Delete the test `sports_url_uses_its_own_host`, from its `#[test]` through its closing brace.

- [ ] **Step 4: Edit `polyoxide-clob/src/ws/client.rs`**

- In the `use super::{...}` block, delete `    sports::SportsMessage,` and change `WS_MARKET_URL, WS_SPORTS_URL, WS_USER_URL,` to `WS_MARKET_URL, WS_USER_URL,`.
- In the `ensure_crypto_provider` doc comment, change `/// channel — market, user and sports alike — abort at connect time for any` to `/// channel — market and user alike — abort at connect time for any`.
- In `parse_channel_message`, replace:

  ```rust
          // The clob channels tag every event with `event_type`, so a frame
          // without one is a heartbeat or a subscription ack. This filter must
          // stay channel-scoped: applying it before the dispatch also silenced
          // the sports channel, and removing it outright would push clob
          // heartbeats into `from_json` and turn them into hard stream errors.
  ```

  with:

  ```rust
          // Both channels tag every event with `event_type`, so a frame
          // without one is a heartbeat or a subscription ack. Removing this
          // filter would push heartbeats into `from_json` and turn them into
          // hard stream errors.
  ```

  and delete:

  ```rust
          // Sports frames carry no discriminator at all — every field is match
          // data. Verified against 229 live frames on 2026-07-25.
          ChannelType::Sports => Ok(Some(Channel::Sports(SportsMessage::from_json(text)?))),
  ```

- In the `require_channel` doc comment, replace the two lines `/// IDs ([`UserSubscriptionUpdate`]) — and the sports channel takes no` and `/// subscription payload at all.` with `/// IDs ([`UserSubscriptionUpdate`]).`
- Delete `connect_sports`: from `    /// Connect to the sports channel for live match updates.` through the closing brace of `pub async fn connect_sports()`, and the blank line after it.
- In the `require_channel` test, delete the third block:

  ```rust
          let err = require_channel(ChannelType::Sports, ChannelType::User).unwrap_err();
          assert!(
              matches!(err, WebSocketError::InvalidMessage(ref m) if m.contains("Sports") && m.contains("User")),
              "the error names both channels: {err}"
          );
  ```

- In `mod dispatch_tests`: delete `use crate::ws::sports::fixtures;`; delete the tests `every_real_sports_frame_reaches_the_caller` and `sports_frames_do_not_yield_market_events` in full; in `skips_keepalive_frames_on_every_channel` change `[ChannelType::Market, ChannelType::User, ChannelType::Sports]` to `[ChannelType::Market, ChannelType::User]`; and in `the_event_type_filter_still_guards_market_and_user` replace the comment's first two lines (`// Scoping the fix to sports must not remove the filter from the` / `// channels that need it: without it, clob heartbeats and subscription`) with `// Without the filter, clob heartbeats and subscription`.

- [ ] **Step 5: Edit `polyoxide-clob/README.md`**

- Change `- **WebSocket**: Real-time market, user, and sports channels (feature-gated)` to `- **WebSocket**: Real-time market and user channels (feature-gated). Live sports scores are in [polyoxide-sports](https://docs.rs/polyoxide-sports/).`
- Delete the `#### Sports Channel` section, from that heading through the closing code fence of its example and the blank line after it, so `#### User Channel` follows the market section directly.

- [ ] **Step 6: Edit `polyoxide-clob/tests/live_ws.rs`**

- Replace the module doc's four lines starting `//! These exist because the sports channel shipped as a public method that` with:

  ```rust
  //! Only a real connection catches a parser that silently discards every frame:
  //! unit tests fed a fabricated frame pass regardless. The sports channel shipped
  //! that way before it moved to `polyoxide-sports`, whose live suite now carries
  //! its tests.
  ```

- Delete `RECV_WINDOW` with its doc line, and both sports tests, from `/// The sports channel must actually deliver parsed frames.` through the closing brace of `live_sports_connection_survives_the_keepalive_interval`.

- [ ] **Step 7: Format, build, and remove anything left unused**

Run:

```bash
cargo fmt --all
cargo clippy -p polyoxide-clob --all-targets --all-features -- -D warnings
```

Expected: clean. If clippy reports an unused import in `live_ws.rs` or `client.rs`, remove that import and rerun.

- [ ] **Step 8: Run clob's tests and check for leftovers**

Run:

```bash
cargo test -p polyoxide-clob --features ws
cargo test -p polyoxide-clob --features ws --test live_ws --no-run
grep -rn 'connect_sports\|SportsUpdateMessage\|SportsMessage\|WS_SPORTS_URL\|ChannelType::Sports\|Channel::Sports' \
  --include='*.rs' --include='*.md' --include='*.py' --include='*.pyi' . \
  | grep -v -e '/target/' -e 'docs/superpowers/' -e 'docs/plans/' -e 'CHANGELOG.md'
```

Expected: tests pass; `live_ws` compiles; the grep prints only lines in `CLAUDE.md`, which Task 18 rewrites.

- [ ] **Step 9: Commit**

```bash
git add -A polyoxide-clob
git commit -F - <<'MSG'
feat(clob)!: move the sports channel to polyoxide-sports

The sports feed is served by sports-api.polymarket.com, takes no
subscription and shares nothing with the order book protocol. It now
lives in its own credential-free crate.

BREAKING CHANGE: WebSocket::connect_sports, Channel::Sports,
ChannelType::Sports, SportsMessage, SportsUpdateMessage and WS_SPORTS_URL
are removed. Use polyoxide_sports::SportsWs for a bare stream, or
polyoxide_sports::SportsWsBuilder for one that reconnects.
SportsUpdateMessage is now polyoxide_sports::MatchUpdate, with the same
fields.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 17: Spec docs under `docs/specs/sports/`

**Files:**
- Move: `docs/specs/clob/asyncapi-sports.json` to `docs/specs/sports/asyncapi.json`
- Create: `docs/specs/sports/INDEX.md`, `docs/specs/sports/OBSERVED.md`
- Modify: `docs/specs/clob/websocket.md`, `docs/specs/clob/INDEX.md`, `docs/specs/INDEX.md`, `docs/specs/gamma/OBSERVED.md`, `SELF-HEALING.md`, `.github/workflows/nightly-schema.yml`

- [ ] **Step 1: Move the mirror and update its one polyoxide reference**

```bash
mkdir -p docs/specs/sports
git mv docs/specs/clob/asyncapi-sports.json docs/specs/sports/asyncapi.json
python3 - <<'EOF'
p = "docs/specs/sports/asyncapi.json"
s = open(p, encoding="utf-8").read()
old = "polyoxide's SportsUpdateMessage is modelled on the observed shape"
assert s.count(old) == 1, s.count(old)
s = s.replace(old, "polyoxide-sports' MatchUpdate is modelled on the observed shape")
open(p, "w", encoding="utf-8").write(s)
EOF
python3 -c 'import json; json.load(open("docs/specs/sports/asyncapi.json"))'
```

Expected: the last command prints nothing, so the file is still valid JSON. That phrase is our annotation, not upstream's text, and this host is excluded from drift checking.

- [ ] **Step 2: Write `docs/specs/sports/INDEX.md`**

```markdown
# Sports feed

Host: `sports-api.polymarket.com`

| Route | Kind | Auth | Implemented by |
|---|---|---|---|
| `wss://…/ws` | WebSocket, server push only, no subscription | None | `polyoxide-sports` (`SportsWs`, `SportsWsBuilder`) |
| `https://…/health` | `GET`, empty `200` | None | Not implemented |

Every other path probed answers `404 page not found`; the list is in
[OBSERVED.md](OBSERVED.md).

Gamma's sports routes (`/sports`, `/sports/market-types`, `/teams`,
`/teams/{id}`) live on `gamma-api.polymarket.com` and are documented in
[../gamma/sports.md](../gamma/sports.md).

| File | What it is |
|---|---|
| [asyncapi.json](asyncapi.json) | Upstream's AsyncAPI document for `/ws`, annotated with `x-observed-payload` and `x-observed-keepalive`. **It does not match the wire**, so it is excluded from `nightly-schema.yml`. Moved from `docs/specs/clob/asyncapi-sports.json` on 2026-10-01. |
| [OBSERVED.md](OBSERVED.md) | What the server actually does |
```

- [ ] **Step 3: Write `docs/specs/sports/OBSERVED.md`**

```markdown
# Sports feed: observed behaviour

`asyncapi.json` beside this file is upstream's AsyncAPI document for
`wss://sports-api.polymarket.com/ws`, annotated with what the server actually
sends. Upstream's document does not match the wire, so the mirror is excluded
from `nightly-schema.yml`. This host's drift detector is the live test
`live_frames_round_trip_and_carry_no_unmodelled_keys` in
`polyoxide-sports/tests/live_api.rs`, which fails and names any key
`MatchUpdate` does not model.

## Contradictions with the published document

| Documented | Observed |
|---|---|
| Payload keyed on `slug`, with `last_update` and `turn` | None of 350 captured frames carried any of them. Frames are keyed on `leagueAbbreviation` plus `gameId`, or `metadataGameId` on cricket |
| Text `"ping"` every 5 s, `"pong"` required within 10 s | WebSocket protocol PING frames every 15.0 s, answered by the transport. No text ping has been seen |
| Message type `sport_result` | Frames carry no discriminator of any kind |

## Captures

| | 2026-07-25 | 2026-10-01, 06:20 UTC |
|---|---|---|
| Duration | 5 min | 5 min |
| Frames | 229 | 121 |
| Leagues | soccer, tennis, cricket, lol, val, cs2, dota2, mlbb | atp, wta, wta challenger, cricket, mlbb, dota2 |
| Protocol pings | 20, about one per 15 s | 19, one every 15.0 s from connect |
| Frames with `eventState` | soccer and tennis | none |
| Longest gap between data frames | not measured | 12.8 s |
| Frames identical to that game's previous frame | not measured | 56 of 121 |

Neither capture overlapped NFL, NBA, MLB or NHL play, or a soccer weekend.
Those leagues' frames are unseen.

## Behaviour a client must allow for

- **Required fields.** `leagueAbbreviation`, `score`, `period`, `live` and
  `ended` were on every frame in both captures. Everything else is optional.
- **Two identifiers.** `gameId`, an integer, on most sports. `metadataGameId`,
  a string beginning `id`, on cricket. No frame carried both.
- **Unchanged state is re-sent.** Each live esports game was re-sent every
  20 s whether or not it changed, and tennis every 30 to 90 s.
- **The ended frame is sent once.** 14 of 15 games that finished during the
  October capture produced exactly one `ended: true` frame. A client that is
  disconnected at that moment never learns the game ended.
- **Cricket ends in sweeps.** 14 cricket matches carried the same
  `finishedTimestamp` to the millisecond (`2026-10-01T06:26:58.54`), most
  never seen live. Cricket's `ended` may mean the feed dropped the match,
  not that it finished.
- **League labels are free text.** `wta challenger` arrived with a space.
- **Status casing varies.** `InProgress`, `inprogress`, `running` and
  `finished` have all been seen.
- **`eventState` comes and goes.** It was on soccer and tennis in July, and
  on no frame in October, tennis included.
- **Silence is normal.** When nothing is live, no data arrives at all. The
  15 s protocol ping is then the only proof of life, which is why the
  supervised stream's 45 s staleness limit counts pings.

## Reconciling through gamma

`GET https://gamma-api.polymarket.com/events?game_id=<gameId>` returned
exactly one event for each of four ids tried on 2026-10-01, including a
finished tennis match showing `ended: true` and its final score.
`game_id=<metadataGameId>` is refused with
`{"type":"validation error","error":"invalid integer"}`, so cricket games
cannot be reconciled this way.

## Routes on the host

`/ws`, and `/health` answering an empty `200`. Every other path probed on
2026-10-01 answered `404 page not found`: `/`, `/ok`, `/status`, `/live`,
`/games`, `/matches`, `/events`, `/v1`, `/v1/live`, `/v1/games`,
`/v1/matches`, `/api`, `/api/live`, `/sports`, `/teams`, `/schedule`,
`/scores`.
```

- [ ] **Step 4: Update `docs/specs/clob/websocket.md`**

- Change the opening schema line's link list from `[asyncapi-market.json](asyncapi-market.json), [asyncapi-user.json](asyncapi-user.json),` / `[asyncapi-sports.json](asyncapi-sports.json).` to `[asyncapi-market.json](asyncapi-market.json), [asyncapi-user.json](asyncapi-user.json).`
- Delete the `| Sports | wss://sports-api.polymarket.com/ws | None |` row from the endpoints table.
- Replace the paragraph `Note the sports channel is on a **different host**...` and the whole `> **The upstream sports contract is wrong.**` block quote with:

  ```markdown
  The live sports feed on `sports-api.polymarket.com` is a separate host and
  protocol. See [../sports/INDEX.md](../sports/INDEX.md).
  ```

- Delete the final `## Sports Channel` section, from the heading to the end of the file.

- [ ] **Step 5: Update the other references**

- `docs/specs/clob/INDEX.md`: change `| [websocket.md](websocket.md) | ws/market, ws/user, ws/sports | Mixed |` to `| [websocket.md](websocket.md) | ws/market, ws/user | Mixed |`.
- `docs/specs/INDEX.md`: in the "Covered by a polyoxide crate" table, add after the Relay row:

  ```markdown
  | [Sports](sports/INDEX.md) | `wss://sports-api.polymarket.com/ws` | Live match scores, server push only | `polyoxide-sports` |
  ```

  and in the WebSocket specs table replace the `clob/asyncapi-sports.json` row with:

  ```markdown
  | [sports/asyncapi.json](sports/asyncapi.json) | Sports feed. **Does not match the wire**; implemented from captured frames by `polyoxide-sports`, see [sports/OBSERVED.md](sports/OBSERVED.md) |
  ```

- `docs/specs/gamma/OBSERVED.md`: change `docs/specs/clob/asyncapi-sports.json` to `docs/specs/sports/asyncapi.json`.
- `SELF-HEALING.md`: change `` **`docs/specs/clob/asyncapi-sports.json`** `` to `` **`docs/specs/sports/asyncapi.json`** ``.
- `.github/workflows/nightly-schema.yml`: change `#   - clob sports AsyncAPI (docs/specs/clob/asyncapi-sports.json):` to `#   - sports AsyncAPI (docs/specs/sports/asyncapi.json):`.

- [ ] **Step 6: Check nothing still points at the old path**

Run:

```bash
grep -rn 'clob/asyncapi-sports\|asyncapi-sports.json' --exclude-dir=target --exclude-dir=.git . \
  | grep -v -e 'docs/superpowers/' -e 'docs/plans/' -e 'polymarket-llms.txt' -e 'docs/specs/sports/INDEX.md'
```

Expected: only `CLAUDE.md` lines, which Task 18 rewrites. The `docs/specs/sports/INDEX.md` mention of the old path is deliberate history. `polymarket-llms.txt` is upstream's index, and it names upstream's file.

- [ ] **Step 7: Commit**

```bash
git add -A docs/specs SELF-HEALING.md .github/workflows/nightly-schema.yml
git commit -F - <<'MSG'
docs(specs): sports feed gets its own spec directory

Mirror moved from docs/specs/clob/; OBSERVED.md records the 2026-07-25
and 2026-10-01 captures, the once-only ended frame, and gamma
reconciliation by game_id.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 18: Release order, nightly row, CLAUDE.md and READMEs

**Files:**
- Modify: `.github/workflows/release.yml`, `.github/workflows/nightly-behavioral.yml`, `CLAUDE.md`, `README.md`, `polyoxide-cli/README.md`

- [ ] **Step 1: Publish order in `release.yml`**

- Change the comment `# Publish in dependency order: core -> rtds -> perps -> relay -> gamma -> data -> clob -> polyoxide` to `# Publish in dependency order: core -> rtds -> sports -> perps -> relay -> gamma -> data -> clob -> polyoxide`.
- Change the `CRATES=(...)` line to:

  ```bash
          CRATES=("polyoxide-core" "polyoxide-rtds" "polyoxide-sports" "polyoxide-perps" "polyoxide-relay" "polyoxide-gamma" "polyoxide-data" "polyoxide-clob" "polyoxide")
  ```

- [ ] **Step 2: Nightly row in `nightly-behavioral.yml`**

Add after the `polyoxide-rtds` row of the matrix:

```yaml
          - { crate: polyoxide-sports, suite: live,        timeout: 15, flags: "--test live_api" }
```

- [ ] **Step 3: Rewrite the CLAUDE.md passages**

Make each change exactly:

1. `Ten crates with this dependency graph:` becomes `Eleven crates with this dependency graph:`.
2. `│   └── polyoxide        (unified client re-exporting clob/gamma/data/rtds/perps, feature-gated)` becomes `│   └── polyoxide        (unified client re-exporting clob/gamma/data/rtds/perps/sports, feature-gated)`.
3. After the line `polyoxide-rtds          (RTDS crypto price streams — depends on NOTHING in-workspace)` add `polyoxide-sports        (live sports scores WebSocket — depends on NOTHING in-workspace)`.
4. In the note under the graph, `` `polyoxide-gamma` and `polyoxide-rtds` — plus `` becomes `` `polyoxide-gamma`, `polyoxide-rtds` and `polyoxide-sports` — plus ``.
5. Directly after that note paragraph, add:

   ```markdown
   The CLI's `ws` group streams `market` and `user` (clob), `prices` (rtds) and `sports`
   (`polyoxide-sports`). `ws sports` takes comma-separated `--league` and `--game` filters and
   `--changes-only`. Its `run_with` takes any event stream, so `polyoxide-cli/tests/ws_sports.rs`
   drives every flag with captured frames.
   ```

6. In the feature-flag sentence, `` `rtds`, `perps`, `full` (all) `` becomes `` `rtds`, `perps`, `sports`, `full` (all) ``.
7. In the nightly-behavioral bullet, `across the five crates with live suites (gamma, data, clob incl. `live_ws` under `--features ws`, relay, cli)` becomes `across every crate with a live suite (gamma, data, clob incl. `live_ws` under `--features ws`, relay, rtds, perps incl. `live_ws`, sports, cli)`.
8. In the **WebSocket** paragraph, replace `Three channels: `WebSocket::connect_market(asset_ids)` (public), `WebSocket::connect_user(condition_ids, credentials)` (authenticated), and `WebSocket::connect_sports()` (public, served by `sports-api.polymarket.com` and taking no subscription payload).` with `Two channels: `WebSocket::connect_market(asset_ids)` (public) and `WebSocket::connect_user(condition_ids, credentials)` (authenticated).`, and replace `The Perps socket (`polyoxide-perps/src/ws/`, feature `ws`) and RTDS (`polyoxide-rtds`) are separate protocols` with `The Perps socket (`polyoxide-perps/src/ws/`, feature `ws`), RTDS (`polyoxide-rtds`) and the sports feed (`polyoxide-sports`) are separate protocols`.
9. `mirrored in `docs/specs/clob/asyncapi-{market,user,sports}.json`` becomes `mirrored in `docs/specs/clob/asyncapi-{market,user}.json` and `docs/specs/sports/asyncapi.json``.
10. Replace the whole paragraph beginning `**The sports mirror does not match the wire.**` with:

    ```markdown
    **The sports feed is its own crate.** `polyoxide-sports` covers `wss://sports-api.polymarket.com/ws`, which takes no subscription and pushes every live match. It depends on nothing in the workspace, for the same reason as rtds. Upstream's AsyncAPI documents a `slug`-keyed payload and a text `"ping"`/`"pong"`; the server sends neither, so `docs/specs/sports/asyncapi.json` carries `x-observed-*` annotations and is excluded from `nightly-schema.yml`. `MatchUpdate` is modelled on the captured frames in `polyoxide-sports/tests/fixtures/` (refresh with `scripts/capture_sports_fixtures.py`), and the live test `live_frames_round_trip_and_carry_no_unmodelled_keys` is the host's drift detector.

    **Pings are the sports feed's liveness signal, the opposite of rtds.** The server sends a protocol PING every 15 s, and data only while a match is live, so a quiet hour has no data at all. `SupervisedSportsWs` resets its 45 s staleness timer on any inbound frame, pings included; counting only data would drop a healthy connection every quiet hour. It is a `Stream` state machine with no background task, because nothing is ever sent to this host, so the perps task shape is not needed. Pongs therefore go out only while the caller polls.

    **A sports frame is state, not an event.** The server re-sends unchanged state on a timer, so about half of all frames are repeats, and the `ended: true` frame is sent once. `Event::Reconnected` tells a caller to reconcile games through gamma's `events?game_id=`; cricket's `metadataGameId` cannot be reconciled. See `docs/specs/sports/OBSERVED.md`.
    ```

11. In **Publishing Order**, `core → rtds → perps → relay → gamma → data → clob → polyoxide` becomes `core → rtds → sports → perps → relay → gamma → data → clob → polyoxide`, and in the parenthetical, after `so its position only has to precede `polyoxide`;` add ` `polyoxide-sports` is the same;`.

- [ ] **Step 4: Root `README.md` crate table**

The table already lacks `polyoxide-perps` and `polyoxide-rtds`; add them with the new crate. Insert after the `polyoxide-gamma` row:

```markdown
| [polyoxide-perps](./polyoxide-perps) | Client library for Polymarket Perps (perpetual futures) public market data |
```

and after the `polyoxide-relay` row:

```markdown
| [polyoxide-rtds](./polyoxide-rtds) | Client for Polymarket's RTDS crypto price streams |
| [polyoxide-sports](./polyoxide-sports) | Client for Polymarket's live sports score feed |
```

- [ ] **Step 5: `polyoxide-cli/README.md`**

Insert after the `ws user` section's closing code fence, before the `---` line:

````markdown

#### `ws sports`

No credentials needed. Reconnects on its own; connection notices go to stderr.

```bash
# Every live match in every league
polyoxide ws sports

# Some leagues, or some games (numeric ids and cricket's id… ids alike)
polyoxide ws sports --league atp,wta
polyoxide ws sports --game 1712005,id2704098174740616

# Drop the server's repeated frames, print JSON, stop after 10 updates
polyoxide ws sports --changes-only --format json -n 10
```
````

- [ ] **Step 6: Check the docs build and nothing stale remains**

Run:

```bash
cargo test -p polyoxide --all-features --doc
grep -n 'connect_sports\|SportsUpdateMessage\|asyncapi-sports' CLAUDE.md
```

Expected: doctests pass (the root README is a doctest); the grep prints nothing.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows CLAUDE.md README.md polyoxide-cli/README.md
git commit -F - <<'MSG'
docs: polyoxide-sports in CLAUDE.md, READMEs, release order and nightly

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01EsF5q2Es2z3F68VQLynqmK
MSG
```

---

### Task 19: The full gate

- [ ] **Step 1: Run every CI step locally, in CI's order**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --all-features --workspace
cargo test --doc --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

Expected: every command succeeds. If `cargo nextest` is not installed, use `cargo test --all-features --workspace` and say so in the report. The doc build matters most: a red one silently withholds the release tag.

- [ ] **Step 2: Run the new crate's suites without features too**

```bash
cargo test -p polyoxide-sports
cargo test -p polyoxide-sports --all-targets
```

Expected: both pass. `bare` and `supervision` are skipped through `required-features` rather than failing to build.

- [ ] **Step 3: Run the live suites once more**

```bash
cargo test -p polyoxide-sports --test live_api -- --ignored --nocapture
cargo test -p polyoxide-cli --test live_api live_ws_sports -- --ignored --nocapture
```

Expected: 6 passed in total. A failure saying "legitimately time out" means nothing was live; rerun later and say so.

- [ ] **Step 4: Confirm the branch is clean and summarise**

Run `git status --short` (expect nothing) and `git log --oneline main..HEAD`. Report the commit list, the gate results, and any live-test outcome that was environmental rather than a pass.

Release is a separate step and is **not** part of this plan. When cutting 0.36.0, the release commit must also bump the `polyoxide-sports` pin in `[workspace.dependencies]`, like every other crate's. The `polyoxide-sports/README.md` install snippet already says `0.36`.
