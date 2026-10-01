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

A third capture, 2026-10-01 at 14:11 UTC, read 176 frames over four minutes
across cs2, challenger, dota2, fif, r6siege, cricket, atp and val. It carried
no `eventState` and no key outside the model. It first showed
`status: not_started`, and soccer (`fif`) frames with `elapsed` but no
`eventState`.

No capture overlapped NFL, NBA, MLB or NHL play, or a soccer weekend. Those
leagues' frames are unseen.

## Behaviour a client must allow for

- **Required fields.** `leagueAbbreviation`, `score`, `period`, `live` and
  `ended` were on every frame in both captures. Everything else is optional.
- **Two identifiers.** `gameId`, an integer, on most sports. `metadataGameId`,
  a string beginning `id`, on cricket. No frame carried both.
- **Unchanged state is re-sent.** Each live esports game was re-sent every
  20 s whether or not it changed, and tennis every 30 to 90 s.
- **The ended frame is usually sent once.** 14 of 15 games that finished during the
  October capture produced exactly one `ended: true` frame. A client that is
  disconnected at that moment never learns the game ended.
- **Cricket ends in sweeps.** 14 cricket matches carried the same
  `finishedTimestamp` to the millisecond (`2026-10-01T06:26:58.54`), most
  never seen live. Cricket's `ended` may mean the feed dropped the match,
  not that it finished.
- **League labels are free text.** `wta challenger` arrived with a space.
- **Status casing varies.** `InProgress`, `inprogress`, `running`,
  `finished` and `not_started` have all been seen.
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
