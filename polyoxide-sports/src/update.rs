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
