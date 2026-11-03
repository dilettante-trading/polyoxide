---
paths:
  - "polyoxide-polymarket/src/sports/**"
  - "polyoxide-polymarket/tests/*sports*.rs"
  - "polyoxide-polymarket/tests/fixtures/sports/**"
  - "polyoxide-cli/src/commands/polymarket/sports.rs"
  - "polyoxide-cli/tests/ws_sports.rs"
  - "scripts/capture_polymarket_sports.py"
  - "docs/specs/sports/**"
---

# Polymarket sports feed

The `sports` module covers `wss://sports-api.polymarket.com/ws`, which takes no subscription and
pushes every live match. Its feature builds `polyoxide-venue` and the socket kit, and not core,
`reqwest` or `alloy`, for the same reason as rtds; `test_dep_allowlists.py` holds `@sports` to
that.

- Upstream's AsyncAPI documents a `slug`-keyed payload and a text `"ping"`/`"pong"`; the server
  sends neither, so `docs/specs/sports/asyncapi.json` carries `x-observed-*` annotations and is
  excluded from `nightly-schema.yml`.
- `MatchUpdate` is modelled on the captured frames in `polyoxide-polymarket/tests/fixtures/sports/`
  (refresh with `scripts/capture_polymarket_sports.py`), which `sports::fixtures` holds for the
  tests here and in the CLI under `test-server`. The live test
  `live_frames_round_trip_and_carry_no_unmodelled_keys` in `tests/live_sports.rs` is the host's
  drift detector.

## Pings are the liveness signal, the opposite of rtds

The server sends a protocol PING every 15 s, and data only while a match is live, so a quiet hour
has no data at all. `SupervisedSportsWs` resets its 45 s staleness timer on any inbound frame,
pings included; counting only data would drop a healthy connection every quiet hour.

On a transport that hides the server's pings (`Capabilities::sees_server_pings` false, a
browser's), `connect` turns the staleness limit off, as `Duration::MAX`, and `stale_after` is
ignored there. `stale_after(Duration::MAX)` turns the check off on any transport, a host's too.
With the limit off no timer is armed at all: the staleness future is
`std::future::pending()`, and an inbound frame re-arms nothing (owner, 2026-11-03). A lost
connection ends through its close event or a failed read, and a half-open browser socket goes
undetected. That is the owner's decision of 2026-11-02 (spec D9), not a
measurement's verdict. The 2026-11 capture (`docs/specs/sports/OBSERVED.md`, "Browser transport")
ran with a game live and saw neither of its two text `ping`s answered; a quiet hour's data gap and
a quiet-hour answer to a text `ping` are unmeasured.

- It is a `Stream` state machine with no background task, because no application message is ever
  sent to this host (only protocol pongs and a close reply). Pongs therefore go out only while the
  caller polls.
- A reconnect refused in a way retrying cannot fix (an HTTP status other than 408, 425, 429 or
  5xx, or a malformed URL) is yielded as `SportsError::Connect` and ends the stream, instead of
  looping at the 60 s backoff ceiling with only a WARN to show for it.
- A private recovery function in `src/sports/supervised.rs` decides (D7 keeps sports off the
  `Supervisor`): a `Capacity` failure and a connection used after it closed (D19) are retried, and
  anything else follows `polyoxide_ws::retrying_can_fix` with polyoxide-venue's status rule
  (`class_for_handshake_status`, which `Classify::is_retriable` reads).

## A sports frame is state, not an event

The server re-sends unchanged state on a timer (56 of 121 frames in one capture were repeats), and
the `ended: true` frame is usually sent once. `Event::Reconnected` tells a caller to reconcile
games through gamma's `events?game_id=` (`polymarket-gamma.md`); cricket's `metadataGameId` cannot
be reconciled. See `docs/specs/sports/OBSERVED.md`.

The nightly also runs this suite at weekends, since 06:00 UTC misses North American leagues and
weekend soccer (`live-tests.md`).
