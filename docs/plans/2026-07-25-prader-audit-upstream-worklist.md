# Upstream worklist from the prader-rs utilization audit (2026-07-25)

Eight defects and gaps in polyoxide 0.22.0, surfaced by an audit of prader-rs — this
SDK's main consumer — against the full 0.22.0 surface. Everything here lives on
polyoxide's side; the consumer-side findings are tracked separately in prader-rs at
`docs/claude/polyoxide-utilization-audit.md` and
`docs/superpowers/specs/2026-07-25-polyoxide-utilization-fixes-design.md`.

This document is written to be handed to a fresh session as its task brief.

---

**IMPORTANT: every claim below was written by an audit agent reading the source. Treat
each as a hypothesis, not a fact.** Open the cited `file:line` and confirm the defect is
real before changing anything. If a claim is wrong, say so and move on — a wrong fix is
worse than none. Several items can only be settled by a live call; those are marked
**LIVE CHECK REQUIRED**, and for those the verification *is* the first deliverable.

---

## Priority 1 — Blocks a consumer feature outright

prader cannot adopt the CLOB WebSocket user channel at all today. It currently finds
order fills by polling REST every 5s and, after 3 consecutive misses, *guesses* that an
order was cancelled externally and records that irreversibly. The user channel would push
`OrderMessage{type: CANCELLATION}` explicitly, plus `size_matched` (partial-fill progress
the consumer currently cannot represent) and `TradeMessage` carrying `taker_order_id` and
`maker_orders[].fee_rate_bps`. Three things block it:

### 1. No dynamic subscribe frame is wrapped

The AsyncAPI spec documents one at `docs/specs/clob/asyncapi-user.json:64`, but
`UserSubscription` is not re-exported from `src/ws/mod.rs:117-126`, so a consumer adding
a market to its watchlist must tear down and reconnect the whole socket.

**Fixed looks like:** the subscription type is public, and there is a method to send a
subscribe/unsubscribe frame on an already-live connection.

### 2. `UserSubscription.markets` is required but is optional upstream

Upstream, omitting the field means "all markets". polyoxide requires it.

**LIVE CHECK REQUIRED:** connect with the field omitted and confirm you receive events for
markets you did not enumerate. If confirmed, make it `Option`. This is the difference
between one socket and one-socket-per-market for every consumer.

### 3. `TradeStatus` transitions are undocumented

The states flow MATCHED → MINED → CONFIRMED, *or* RETRYING → FAILED — so MATCHED is **not**
terminal. A consumer treating it as final will book fills that later fail.

**Fixed looks like:** the state machine documented on the type itself, not only in the spec
mirror.

## Priority 2 — Silent 45x rate-limit over-permit on live traffic

### 4. `RateLimiter::clob_default()` omits `/balance-allowance`

`polyoxide-core/src/rate_limit.rs:133-170`. The endpoint falls through to the general
9,000/10s bucket while Polymarket documents 200/10s — so the limiter reports compliant
while permitting roughly 45x the allowed rate. It is prader's hottest authenticated
endpoint.

Same function, same class of bug:

- `/tick-size` is permitted at 1,500/10s against a documented 200/10s.
- Batch patterns are missed entirely — `/books` does not match the `/book` prefix rule at
  `:98-107`, so batch reads are unlimited.

**Cross-check every entry against `docs/specs/clob/rate-limits.md`** rather than fixing
only the three named here. If the table has drifted once it has likely drifted more.

## Priority 3 — A public API that cannot work

### 5. The sports WebSocket channel yields an empty stream

`connect_sports` is public and carries a doc example, but:

- `parse_channel_message` (`src/ws/client.rs:40-44`) drops any frame not containing the
  literal substring `event_type`;
- `SportsUpdateMessage.event_type` (`src/ws/sports.rs:15-17`) is a required `String`;
- the real payload has no such field — `docs/specs/clob/asyncapi-sports.json`
  `SportResult` requires only `slug`.

So every frame is discarded and the stream is silently empty. Separately, the channel's
keep-alive is a **text** `ping` requiring a text `pong`, and there is no handler for it.

**Two things make this larger than a substring check — both verified against the source:**

- **The `event_type` filter is shared by all three channels, not sports-specific.** It sits
  at `ws/client.rs:41-44`, *before* the `match channel_type` dispatch, and the same function
  serves both `WebSocket` and `WebSocketWithPing` (`:239`, `:473`). Its real job is skipping
  heartbeats and acks. Removing it outright to unblock sports would push market-channel
  heartbeats into `MarketMessage::from_json` and turn them into hard errors. **Any fix must
  be channel-scoped.**

- **The existing sports tests are built on a fabricated fixture, which is how this survived.**
  `const SPORTS` (`ws/client.rs:544`) is `{"event_type":"sports_update","game_id":"g-1"}` — it
  carries the very field the real payload lacks, and `game_id` is not in `SportResult` either
  (which requires only `slug`). Two tests assert against it — `routes_by_channel_type` and
  `sports_frames_do_not_parse_as_market_frames` — and both pass while verifying the parser
  handles a frame the venue never sends.

**LIVE CHECK REQUIRED**, and it is the first step, not a validation afterthought: connect and
capture real frames, replace the `SPORTS` fixture with a captured one, watch the tests fail,
then fix parser and message type to match — leaving the shared pre-filter intact for market
and user. Add the text-ping handler.

**If the endpoint is dead upstream, removing `connect_sports` is a better outcome** than a
public method that returns nothing.

## Priority 4 — Typing and data corrections

### 6. `holders` limit range contradicts the spec

Typed 0-500 default 100 (`polyoxide-data/src/api/holders.rs:30-31`); the mirrored spec says
0-20 default 20 (`docs/specs/data/holders.md:16`).

**LIVE CHECK REQUIRED:** call with `limit=100` and see whether it 400s. Correct whichever
side is wrong. If the spec is right, the current typing invites a runtime 400 that the type
system implies is impossible.

### 7. `Tags::get_related_by_slug` is probably mistyped

Returns `Vec<Tag>` (`polyoxide-gamma/src/api/tags.rs:45-56`), but the spec says the endpoint
returns relationship rows (`docs/specs/gamma/tags.md:100`), and `Tag` requires non-Option
`id`/`slug`/`label` — so a relationship row would fail to deserialize. Verify against a live
response. `get_related_detailed_by_slug` (`tags.rs:69-81`) may already be the correct shape.

### 8. `LeaderboardCategory` omits ESPORTS

`polyoxide-data/src/api/leaderboard.rs:74-117` vs `docs/specs/data/leaderboard.md:15`. Small,
mechanical. Note that Geopolitics and Science have no upstream category either, so check
whether the enum is systematically stale rather than missing exactly one variant.

## Explicitly NOT in scope

- **RFQ** — removed from the SDK in 0.17.0 though the endpoints remain documented and
  `ClobMarketDetails.rfqe` still reports eligibility. Leave it.
- **A merge/split wrapper for CTF positions** — `Position.mergeable` exists but the only
  route is hand-encoded `mergePositions` calldata via `RelayClient::execute`
  (`polyoxide-relay/src/client.rs:573`). That is a feature build, not a fix; do not start
  it here.

## Conventions

- MSRV 1.91. `cargo clippy --all-targets --all-features -- -D warnings` must be clean and
  `cargo fmt --all -- --check` must pass — CI treats all warnings as errors.
- Live tests are `#[ignore]`d and run with
  `cargo test -p polyoxide-clob --test live_api -- --ignored`. **Add the live checks above
  as ignored tests** where they make sense, so the next person can re-run them instead of
  re-deriving them.
- Items 1, 2 and 6 are breaking. **Group breaking work into one release** rather than
  dribbling it across several: the main consumer pins all six crates in lockstep from a
  single workspace table and bumps them together.
- Update `CHANGELOG.md`; mark breaking entries `[**breaking**]` as existing entries do.

## Deliverable

For each of the eight items: whether you confirmed the defect, what you changed (or why you
didn't), and — for the four needing live calls — what the venue actually returned.

Call out anything that turns out to be a **consumer-side misunderstanding rather than an SDK
defect**, since prader has a spec written against these assumptions and will need
correcting.
