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
            assert!(
                !frame.contains('\n'),
                "{name} is not a single captured line"
            );
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
