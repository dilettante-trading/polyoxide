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
| `elapsed_without_event_state.json` | 2026-10-01 | Soccer (`fif`) with `elapsed` but no `eventState` |

## Captures

- **2026-07-25**, 229 frames over five minutes, covering soccer, tennis,
  cricket and the lol, val, cs2, dota2 and mlbb esports titles. These five
  frames were first held as constants in `polyoxide-clob/src/ws/sports.rs`.
- **2026-10-01, 06:20 UTC**, 121 frames over five minutes across atp, wta,
  wta challenger, cricket, mlbb and dota2. No frame carried `eventState`.
  Protocol pings arrived every 15.0 s, and no text ping was sent.
- **2026-10-01, 14:11 UTC**, 176 frames over four minutes across cs2,
  challenger, dota2, fif, r6siege, cricket, atp and val. No frame carried
  `eventState` or a key `MatchUpdate` does not model. It first showed
  `status: not_started`. Its one new key-set, `elapsed` without
  `eventState`, is kept as `elapsed_without_event_state.json`.
