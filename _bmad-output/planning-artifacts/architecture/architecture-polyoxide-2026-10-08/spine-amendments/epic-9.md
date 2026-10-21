# Proposed spine amendments: Epic 9

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A9-1: the Kalshi crate is published and in the umbrella (amends AD-16, and the bindings of AD-7 and AD-11)

- **Proposed by:** the owner, 2026-10-19, during Epic 9's design (spec §1.3, D12 and D13, in `docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design.md`).
- **Current rule:**
  - AD-16 lists, among S3's additions, "the Kalshi skeleton, `publish = false` until Kalshi integration".
  - AD-7 and AD-11 bind "the Kalshi skeleton".
- **Proposed rule:**
  - S3's additions list the Kalshi venue crate, published and re-exported by the umbrella as `polyoxide::kalshi`, and AD-7 and AD-11 bind the Kalshi crate.
  - The crate drops `publish = false`. The umbrella gains `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws`, all in `full` and none in its default.
- **Why:**
  - **The skeleton did its job.** Epic 8 built it to show that a venue lands by the guide's recipe. Epic 9 fills the market-data gaps it deferred, so the first version on crates.io is the complete one.
  - **"Integration" is the umbrella.** A venue that the umbrella re-exports is how a consumer of both venues reaches it, and the guide's recipe already ends there.
- **Follow-up in the code:** Story 8 drops `publish = false`, gives the workspace pin a version, adds the umbrella's features, passthrough, `parse_key` arm and prelude names, and the dependency-allowlist rules. The release commit, `0.42.0`, is an owner session after the run (spec §12 and D16). The merge session edits AD-16, AD-7 and AD-11, and the guide's text.

## A9-2: `Candles::intervals()` and the refusal of an unserved width (resolves A8-3, amends AD-6)

- **Proposed by:** the owner, 2026-10-19, who chose the form (spec §1.3, D2 and D3).
- **Current rule (AD-6):** optional abilities are separate traits, implemented only where offered, and no method returns `Unsupported`. `CandleInterval` names one minute to one week, "each width every candle venue serves". A8-3 left the choice between a venue declaring its widths and a venue refusing the others.
- **Proposed rule:**
  - Both, in the form that adds no method answering "unsupported". `Candles` has a provided method, `intervals(&self) -> &'static [CandleInterval]`, whose default is `CandleInterval::ALL`, so a caller can ask before it calls and a venue that serves fewer widths overrides it.
  - A query whose width is outside `intervals()` is refused as `InvalidRequest` with nothing sent, as a key of another product is. `CandleQuery::ensure_served` makes the refusal, `UnservedInterval`, which implements `Classify` as `InvalidRequest` and a fault, and an implementation converts it to `ClassifiedError` with `?`.
  - The CLOB, perps and USDⓈ-M keep the default. Kalshi serves one minute, one hour and one day.
  - `CandleInterval`'s "each width every candle venue serves" no longer holds, and the guide and the traits' docs stop saying it.
- **Why:**
  - **Kalshi serves three widths.** Its batch candlestick route answers `period_interval=5` with a `200` and the daily candles (`docs/specs/kalshi/OBSERVED.md`, Story 1's capture), so an unserved width would silently read as daily. The venue does not guard the width, and the refusal is the only guard.
  - **A provided method is additive.** Every implementation written before it keeps its behaviour, and the change is semver-minor.
  - **A refused request is not an unsupported operation.** The venue offers `Candles`; the caller asked for a width it does not serve.
- **Follow-up in the code:** Story 2 adds the method, `ensure_served` and `UnservedInterval` to `polyoxide-venue`, and Story 3 implements `Candles` for Kalshi. The merge session edits AD-6 and the guide.

## A9-3: `Protocol::follow_up` lets a protocol send a frame from `decode` (amends AD-11 and the guide's step 4)

- **Proposed by:** Epic 9's design, 2026-10-19 (spec D7 and D9).
- **Current rule:** AD-11 says a venue implements only a `Protocol`, and lists its parts. Every frame a protocol sends is the handshake, a ping, or a membership frame from `replay` or `apply`. `decode` returns events and sends nothing.
- **Proposed rule:**
  - The list gains an optional part, `follow_up`: one frame the protocol sends after a `decode` that leaves the connection up. By default there is none.
  - The `Supervisor` calls it once after each such `decode` and sends the frame it returns, bounded by `stale_after` and without awaiting an answer, which comes back through `decode`. A send that fails or times out ends the connection as a failed ping does. A frame the protocol still holds when the connection ends stays with the protocol, which clears it in `reset`.
- **Why:**
  - **Kalshi's recovery from a gap needs it.** A gap in the order-book sequence is seen in `decode`, and the cheaper recovery is a `get_snapshot` on the same connection. `replay` and `apply` run only for a membership change, and a ping only on its schedule, so reconnecting was the only recovery the kit allowed.
  - **It is additive and venue-neutral.** The default changes no existing protocol, and the kit's docs name no venue.
- **Follow-up in the code:** Story 4 adds the method and its call, with five tests in `polyoxide-ws/src/supervisor/tests.rs`, and Story 5 uses it for the re-snapshot. The merge session edits AD-11 and the guide.

## A9-4: a venue's empty membership must not subscribe (amends AD-11)

- **Proposed by:** Epic 9's design, 2026-10-19 (spec D8).
- **Current rule:** AD-11's membership rules say that Binance answers false on empty, that perps and the clob market channel answer true, and that the clob user channel's membership is an `Option` so that an empty `Some` never collapses into `None`, which means every market. They do not say what a venue sends for an empty membership.
- **Proposed rule:**
  - A venue's empty membership must not subscribe, since a venue may refuse an empty subscription or read it as every market. Its protocol sends no `subscribe` while the membership is empty, sends an `unsubscribe` for every subscription when the membership empties, and sends a fresh `subscribe` at the first add.
  - The kit is unchanged: the rule is the protocol's `replay` and `apply`.
- **Why:**
  - **Kalshi refuses an empty subscribe on two channels.** It refuses a `ticker` or `trade` subscribe with no markets (code 14, "Market Ticker required", on the demo host, 2026-10-21; `docs/specs/kalshi/OBSERVED.md`, Epic 9). The design first read such a subscribe as every market, and the probe corrected it. Either way a replay of an empty membership must not send one: a refusal would end the stream for every ticker, and a venue that did read it as every market would subscribe the consumer to the whole exchange. AD-11's "Prevents" list already names widened subscriptions.
  - **Epic 8 had the rule for one channel.** Its protocol sent no `subscribe` for an empty order-book membership; Epic 9 makes that the rule for all three channels.
- **Follow-up in the code:** Story 5 applies the rule to all three channels, and its supervision tests pin the empty replay and the first add. The merge session adds the rule to AD-11.

## A9-5: a protocol cannot end its connection from `apply` (amends AD-11)

- **Proposed by:** Epic 9's Story 5 follow-up, 2026-10-21 (spec D8, "A refusal partway"; commit 2695141).
- **Current rule:** AD-11's `Protocol` describes a membership change as `apply`, which sends the change's frames over the connection's request correlation and returns an error when the venue refuses. The error fails the change. Nothing in `apply`'s contract lets a protocol say that the connection itself must be replaced.
- **Proposed rule:**
  - `apply`'s error, or a provided method beside it, can ask the `Supervisor` to reconnect: a protocol whose change failed after the venue acted on part of it (Kalshi sends one `update_subscription` for each channel's `sid`, and can refuse the second after accepting the first) ends the connection at once, and the replay brings the venue's subscriptions and the membership back into agreement.
  - The kit's behaviour for a protocol that does not ask is unchanged.
- **Why:**
  - **The workaround ends the connection late.** Kalshi's protocol sets a flag when a refusal follows an accepted command, and its next `decode` raises that flag as an error for `recover` to answer `Reconnect`. The connection therefore ends only when the next frame is read, up to one ping interval later, and until then the venue's subscriptions and the membership disagree.
  - **The flag needs care.** It lives across `apply` and `decode`, and `reset` has to clear it for the next connection; each such protocol repeats that pairing.
- **Follow-up in the code:** commit 2695141 holds the workaround, and its tests pin the flag's life. The merge session decides whether the kit grows the request, and edits AD-11 and the guide.
