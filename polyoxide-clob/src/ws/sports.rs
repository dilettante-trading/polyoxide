//! Sports channel message types.
//!
//! The sports channel differs from market and user in two ways: it lives on
//! the `sports-api` host rather than `ws-subscriptions-clob`, and it takes no
//! subscription payload — connecting is enough to start receiving updates.

use serde::{Deserialize, Serialize};

/// A live sports match update.
///
/// # Field requiredness
///
/// Modelled from 229 frames captured on 2026-07-25 across soccer, tennis,
/// cricket and five esports titles. Only the fields present in *every* frame
/// are required; everything else is optional, because the channel is markedly
/// heterogeneous — five distinct key-sets appeared in a single five-minute
/// window.
///
/// In particular `game_id` is **not** required: cricket frames identify the
/// match with [`metadata_game_id`](Self::metadata_game_id) instead and carry no
/// `gameId`, `homeTeam`, `awayTeam` or `status` at all.
///
/// Frames carry no event discriminator — there is no `event_type` field on this
/// channel, unlike the market and user channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SportsUpdateMessage {
    /// League or competition code, e.g. `"kor"`, `"atp"`, `"lol"`, `"cricket"`.
    pub league_abbreviation: String,
    /// Current score. Format is sport-specific — `"2-1"` for soccer,
    /// `"4-6, 1-2"` for tennis, `"000-000|0-1|Bo3"` for esports.
    pub score: String,
    /// Current period, e.g. `"2H"`, `"S2"`, `"1/1"`, `"Live"`, `"FT"`.
    pub period: String,
    /// Whether the match is in progress.
    pub live: bool,
    /// Whether the match has finished.
    pub ended: bool,
    /// Numeric match identifier. Absent on cricket frames — see
    /// [`metadata_game_id`](Self::metadata_game_id).
    #[serde(default)]
    pub game_id: Option<u64>,
    /// String match identifier used in place of [`game_id`](Self::game_id) on
    /// cricket frames.
    #[serde(default)]
    pub metadata_game_id: Option<String>,
    /// Home team or first player. Absent on cricket frames.
    #[serde(default)]
    pub home_team: Option<String>,
    /// Away team or second player. Absent on cricket frames.
    #[serde(default)]
    pub away_team: Option<String>,
    /// Venue-side status string, e.g. `"InProgress"`, `"inprogress"`,
    /// `"running"`. Casing is not normalised upstream.
    #[serde(default)]
    pub status: Option<String>,
    /// Elapsed time within the period, `"MM:SS"` or minutes. Often absent.
    #[serde(default)]
    pub elapsed: Option<String>,
    /// When the match ended. Only present once `ended` is true.
    #[serde(default)]
    pub finished_timestamp: Option<String>,
    /// Nested per-sport detail. Shape varies by sport — tennis adds
    /// `tournamentName` and `tennisRound`, for instance — so it is left as raw
    /// JSON rather than guessed at.
    #[serde(default)]
    pub event_state: Option<serde_json::Value>,
    /// Any field this type does not model, retained rather than dropped.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Sports channel message types
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum SportsMessage {
    /// A live game state update.
    Update(SportsUpdateMessage),
}

impl SportsMessage {
    /// Parse a sports channel message from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        Ok(SportsMessage::Update(serde_json::from_str(json)?))
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    //! Frames captured verbatim from `wss://sports-api.polymarket.com/ws` on
    //! 2026-07-25 over a five-minute window (229 frames total).
    //!
    //! These replace a fabricated fixture that carried an `event_type` field
    //! the venue has never sent. That invention is why the sports channel
    //! shipped broken with passing tests: the parser filtered on `event_type`,
    //! and the only frame the tests fed it was one that happened to have one.
    //!
    //! The five constants below are the five *distinct key-sets* observed in
    //! that window — the channel is far more heterogeneous than one example
    //! suggests, so tests should exercise all of them.

    /// Soccer, with the nested per-sport `eventState` block.
    pub const SOCCER: &str = r#"{"gameId":90106111,"leagueAbbreviation":"kor","homeTeam":"Gimcheon Sangmu FC","awayTeam":"Daejeon Hana Citizen FC","status":"InProgress","eventState":{"type":"soccer","createdAt":"2026-07-25T12:02:18.395759286Z","updatedAt":"2026-07-25T12:02:18.395759286Z","score":"2-1","elapsed":"74","period":"2H","live":true,"ended":false},"score":"2-1","elapsed":"74","period":"2H","live":true,"ended":false}"#;

    /// Tennis — `eventState` carries fields no other sport sends.
    pub const TENNIS: &str = r#"{"gameId":5968822,"leagueAbbreviation":"atp","homeTeam":"Alexander Bublik","awayTeam":"Quentin Halys","status":"inprogress","eventState":{"type":"tennis","createdAt":"2026-07-25T12:02:23.120698555Z","updatedAt":"2026-07-25T12:02:23.120698555Z","score":"4-6, 1-2","period":"S2","live":true,"ended":false,"tournamentName":"Generali Open","tennisRound":"Final"},"score":"4-6, 1-2","period":"S2","live":true,"ended":false}"#;

    /// Esports — the most common shape; no `eventState`, no `elapsed`.
    pub const ESPORTS: &str = r#"{"gameId":1590176,"leagueAbbreviation":"lol","homeTeam":"Caldya Esport","awayTeam":"Galions Sharks","status":"running","score":"000-000|0-0|Bo1","period":"1/1","live":true,"ended":false}"#;

    /// Cricket — identified by `metadataGameId`, with **no** `gameId`,
    /// `homeTeam`, `awayTeam` or `status` at all.
    pub const CRICKET: &str = r#"{"metadataGameId":"id2703680373085574","leagueAbbreviation":"cricket","score":"21-178","period":"Live","live":true,"ended":false}"#;

    /// A finished match, the only shape carrying `finishedTimestamp`.
    pub const CRICKET_FINISHED: &str = r#"{"metadataGameId":"id2703438269077680","leagueAbbreviation":"cricket","score":"116-38","period":"FT","live":false,"ended":true,"finishedTimestamp":"2026-07-25T12:06:54.595448902Z"}"#;

    /// Every distinct shape, for tests that must cover all of them.
    pub const ALL: [&str; 5] = [SOCCER, TENNIS, ESPORTS, CRICKET, CRICKET_FINISHED];
}

#[cfg(test)]
mod tests {
    use super::{fixtures, SportsMessage};

    #[test]
    fn every_captured_frame_shape_parses() {
        for (i, frame) in fixtures::ALL.iter().enumerate() {
            let SportsMessage::Update(msg) = SportsMessage::from_json(frame)
                .unwrap_or_else(|e| panic!("fixture {i} must parse: {e}\n{frame}"));
            assert!(
                !msg.league_abbreviation.is_empty(),
                "fixture {i} lost its league"
            );
        }
    }

    #[test]
    fn parses_a_soccer_frame() {
        let SportsMessage::Update(msg) = SportsMessage::from_json(fixtures::SOCCER).unwrap();
        assert_eq!(msg.game_id, Some(90106111));
        assert_eq!(msg.league_abbreviation, "kor");
        assert_eq!(msg.home_team.as_deref(), Some("Gimcheon Sangmu FC"));
        assert_eq!(msg.score, "2-1");
        assert_eq!(msg.period, "2H");
        assert_eq!(msg.elapsed.as_deref(), Some("74"));
        assert!(msg.live);
        assert!(!msg.ended);
        // Per-sport detail is retained rather than guessed at.
        assert_eq!(msg.event_state.as_ref().unwrap()["type"], "soccer");
    }

    #[test]
    fn parses_a_cricket_frame_that_has_no_game_id() {
        // Cricket is the shape that proves `gameId` cannot be required: a
        // 30-frame sample showed it on 30/30, but it is absent here.
        let SportsMessage::Update(msg) = SportsMessage::from_json(fixtures::CRICKET).unwrap();
        assert_eq!(msg.game_id, None);
        assert_eq!(msg.metadata_game_id.as_deref(), Some("id2703680373085574"));
        assert_eq!(msg.home_team, None);
        assert_eq!(msg.status, None);
        assert_eq!(msg.league_abbreviation, "cricket");
    }

    #[test]
    fn parses_a_finished_match() {
        let SportsMessage::Update(msg) =
            SportsMessage::from_json(fixtures::CRICKET_FINISHED).unwrap();
        assert!(msg.ended);
        assert!(!msg.live);
        assert_eq!(
            msg.finished_timestamp.as_deref(),
            Some("2026-07-25T12:06:54.595448902Z")
        );
    }

    #[test]
    fn retains_fields_the_type_does_not_model() {
        // Upstream adds sports over time; an unmodelled field must survive
        // rather than be silently dropped.
        let json = r#"{"leagueAbbreviation":"nfl","score":"14-7","period":"Q2",
            "live":true,"ended":false,"turn":"sea","somethingNew":42}"#;
        let SportsMessage::Update(msg) = SportsMessage::from_json(json).unwrap();
        assert_eq!(msg.extra["turn"], "sea");
        assert_eq!(msg.extra["somethingNew"], 42);
    }

    #[test]
    fn rejects_a_frame_that_is_not_a_sports_update() {
        // The channel carries nothing but match updates, so a frame missing the
        // universal fields is a real error, not something to swallow.
        assert!(SportsMessage::from_json(r#"{"some":"ack"}"#).is_err());
    }
}
