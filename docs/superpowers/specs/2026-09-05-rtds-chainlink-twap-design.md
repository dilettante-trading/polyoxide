# RTDS crypto price streams (`polyoxide-rtds`)

**Date:** 2026-09-05
**Status:** Approved, not yet implemented
**Upstream:** <https://docs.polymarket.com/market-data/chainlink-twap>, <https://docs.polymarket.com/market-data/realtime-data>

## Problem

Polymarket's Real-Time Data Service (RTDS, `wss://ws-live-data.polymarket.com`)
carries reference prices that no polyoxide crate covers: Binance spot, Chainlink
spot, and Chainlink-computed 30-second and 60-second TWAPs. Consumers trading
15-minute crypto markets need the TWAP feeds, since those are the values the
markets resolve against.

RTDS is a different protocol from the three WebSocket channels in
`polyoxide-clob/src/ws/`. Those are one-connection-per-channel, each sending a
bare subscription object. RTDS multiplexes many topics over one connection,
wrapped in an `action`/`subscriptions` envelope, and tags every frame with
`topic` and `type`.

## Scope

**In scope:** the RTDS transport, and the four crypto price topics.

| Wire topic | Meaning |
|---|---|
| `crypto_prices` | Binance spot |
| `crypto_prices_chainlink` | Chainlink spot |
| `crypto_prices_twap_thirty` | Chainlink 30s TWAP |
| `crypto_prices_twap_sixty` | Chainlink 60s TWAP |

**Out of scope, deliberately:** the `equity_prices` and `comments` topics. Both
ride the same transport and are cheap follow-ups. `comments` is deferred because
its payload overlaps `polyoxide-gamma`'s existing comment types, which have
their own history of not matching spec or wire; merging that question into this
one would couple two unrelated risks.

**Also out of scope:** direct Chainlink Data Streams access
(`api.dataengine.chain.link`). It requires Chainlink credentials, signed
requests, a clock within five seconds of Chainlink's, and DON signature
verification. RTDS relays the same values without credentials.

## Evidence base

Every design decision below traces to behaviour observed live on 2026-09-05
across seven probes against `wss://ws-live-data.polymarket.com`. **The published
documentation is wrong in six places**, and the design exists largely to make
those six unable to hurt a caller.

| # | Documented | Observed |
|---|---|---|
| 1 | `full_accuracy_value` is "the exact signed E18 fixed-point value" | E18 on the three Chainlink topics; a **plain decimal** on `crypto_prices`. `"79697.73000000"` vs `"79696948174287960000000"`, same field, same envelope, ~1s apart |
| 2 | TWAP: "no snapshot, history, or replay" | Every topic sends a `type:"subscribe"` backfill first. Chainlink topics ~55–59 points (~1 min at 1s cadence); Binance 120 points (~2 min) |
| 3 | Binance filter is `"btcusdt,ethusdt"` (comma-separated) | Yields **zero frames**. Working form is `{"symbol":"btcusdt"}` — the same JSON form as Chainlink |
| 4 | Symbols must be lowercase | `{"symbol":"BTC/USD"}` works; matching is case-insensitive |
| 5 | Envelope is `{topic,type,timestamp,payload}` | `update` frames carry an undocumented `connection_id`; `subscribe` frames do not |
| 6 | Chainlink supports 4 symbols; Binance 4 | 8 Chainlink live (`btc eth sol xrp bnb doge hype zec`), 6 Binance (`btc eth sol xrp bnb doge`) |

Three further behaviours are undocumented entirely:

**The whitespace trap.** A filter with one stray space —
`{"symbol": "btc/usd"}` instead of `{"symbol":"btc/usd"}` — still delivers the
subscribe backfill, then goes permanently silent on updates. No error. The
failure is indistinguishable from an idle feed.

**Batch poisoning.** One unrecognised topic in a subscription array returns zero
frames on *every* topic in that array, plus a single frame in a disjoint
envelope: `{"body":{"message":"leger GetTopics error: … not found"},"statusCode":401}`.
The status is `401` on what is actually a not-found.

**`PING` is not load-bearing, and there is no `PONG`.** With both the
application `PING` and the library's protocol-level ping disabled, a
subscription ran 240 seconds and 224 frames, still flowing when cut. Across all
probes the only non-JSON text frame RTDS ever sent was a single empty string at
connect time — never a reply to a ping. The heartbeat is write-only and yields
no liveness signal.

Per-topic shapes, as observed:

| Topic | update `full_accuracy_value` | `window_s` | snapshot points | exact value in snapshot |
|---|---|---|---|---|
| `crypto_prices` | plain decimal | absent | `{timestamp, value}` | **no** |
| `crypto_prices_chainlink` | E18 | absent | `{timestamp, value}` | **no** |
| `crypto_prices_twap_thirty` | E18 | `30` | `{timestamp, value, full_accuracy_value}` | yes |
| `crypto_prices_twap_sixty` | E18 | `60` | `{timestamp, value, full_accuracy_value}` | yes |

Symbol sets moved beyond the documented four within one observation window, so
**symbols are `String`, never an enum**. An enum would be wrong within weeks and
would reject a valid symbol as a hard error.

## Architecture

### Crate

A new leaf crate, `polyoxide-rtds`, with **no dependency on `polyoxide-core`**.
`polyoxide-clob`'s `WebSocketError` already demonstrates that a WebSocket client
here needs only `thiserror`, `tungstenite`, `serde_json` and `url`; core is
reached solely by the keychain auth path, which a credential-free feed does not
have.

The alternative — a module inside `polyoxide-clob/src/ws/` — was rejected on
dependency weight. `alloy` is a mandatory dependency of `polyoxide-clob`
(`polyoxide-clob/Cargo.toml:26`) because order signing needs EIP-712. Measured:
`polyoxide-clob --features ws` builds 352 crates against `polyoxide-core`'s 161.
A caller who wants nothing but a BTC/USD TWAP would compile the entire Ethereum
signing stack for a feed that uses no credentials and signs nothing.

No feature gates: the crate *is* a WebSocket client, so its ws dependencies are
unconditional. Dependencies (`tokio-tungstenite`, `futures-util`, `serde`,
`serde_json`, `rust_decimal`, `thiserror`, `tracing`, `tokio`, `rustls`, `url`)
are all already workspace dependencies.

Because nothing in the workspace depends on it except the unified `polyoxide`
crate, its position in the publish order is a readability choice, not a
correctness one. Slot it after `polyoxide-core`.

### Module layout

```
polyoxide-rtds/
├── src/
│   ├── lib.rs           re-exports, crate docs
│   ├── topic.rs         Topic, TwapWindow
│   ├── subscription.rs  Subscription, SubscriptionRequest
│   ├── event.rs         PriceEvent, PriceUpdate, per-topic payloads, Snapshot
│   ├── client.rs        Rtds — tier 1, bare Stream
│   ├── supervisor.rs    RtdsBuilder, SupervisedRtds — tier 2
│   └── error.rs         RtdsError
├── examples/
│   └── twap_stream.rs
└── tests/
    ├── scripted_server.rs   local tokio-tungstenite harness (dev-only)
    ├── supervision.rs       reconnect / staleness / resubscribe, against the harness
    └── live_api.rs          #[ignore], real host
```

### Protocol layer

**`Topic` is a closed `#[non_exhaustive]` enum, not a string.** Batch poisoning
means one bad topic name silently zeroes an entire subscription batch; making an
invalid topic unrepresentable removes the failure mode rather than documenting
it. `#[non_exhaustive]` so `equity_prices` and `comments` can be added later
without a breaking change.

```rust
#[non_exhaustive]
pub enum Topic {
    BinanceSpot,                 // crypto_prices
    ChainlinkSpot,               // crypto_prices_chainlink
    ChainlinkTwap(TwapWindow),   // crypto_prices_twap_{thirty,sixty}
}

pub enum TwapWindow { Thirty, Sixty }   // .seconds() -> 30 | 60
```

**The `filters` string is never caller-supplied.** `Subscription` holds an
`Option<String>` symbol and serialises the filter itself via
`serde_json::to_string` on a one-field struct, which is compact by construction.
The whitespace trap therefore cannot be reached from the public API — the bug
only exists for callers writing the filter literally, so the fix is to make the
raw string inaccessible rather than to validate it.

**Multi-symbol fans out.** `{"symbol":["btc/usd","eth/usd"]}` returned zero
frames; one symbol per subscription entry is the only working form. So
`.symbols(["btc/usd", "eth/usd"])` expands to **two** subscription entries. This
is invisible to the caller and is the difference between a working feed and
silence.

```rust
let subs = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
    .symbols(["btc/usd", "eth/usd"]);   // -> 2 entries
let all  = Subscription::for_topic(Topic::ChainlinkSpot);  // no filter, all symbols
```

**The error envelope is parsed, not dropped.** `{"body":{…},"statusCode":N}`
shares no keys with `{topic,type,timestamp,payload}` and must surface as a
stream item, or a rejected subscription is indistinguishable from an idle feed.

**Frames skipped:** the empty text frame sent at connect, and any `PONG` —
the same treatment `parse_channel_message` gives them today
(`polyoxide-clob/src/ws/client.rs:58`).

**No rate limiting.** Upstream publishes none for RTDS and none was observed.
Recorded as a decision, not an omission: this is deliberately unlike
`polyoxide-core/src/rate_limit.rs`.

### Event types

Per-topic structs inside an enum, following the `market.rs` precedent — seven
per-event structs there share `event_type`/`asset_id`/`market`/`timestamp` and
are still kept separate rather than merged into one struct of `Option`s.

```rust
#[non_exhaustive]
pub enum PriceEvent {
    Update(PriceUpdate),
    /// The backfill RTDS sends once, immediately after each subscribe.
    Snapshot(Snapshot),
}

#[non_exhaustive]
pub enum PriceUpdate {
    Binance(BinanceUpdate),             // full_accuracy_value is a PLAIN decimal
    ChainlinkSpot(ChainlinkSpotUpdate), // full_accuracy_value is E18
    Twap(TwapUpdate),                   // E18, plus a window
}

impl PriceUpdate {
    pub fn topic(&self) -> Topic;
    pub fn symbol(&self) -> &str;
    pub fn observed_at(&self) -> i64;   // venue observation time (payload.timestamp)
    pub fn published_at(&self) -> i64;  // RTDS receipt time (outer timestamp)
    pub fn value(&self) -> Decimal;     // total: every update has an exact value
    pub fn window(&self) -> Option<TwapWindow>;
}
```

The two-level split is deliberate. Uniform accessors live on `PriceUpdate`,
where `value()` is **total**; putting them on `PriceEvent` would force
`value()` to return `Option` because a `Snapshot` is many points, and two of the
four topics carry no exact values in their backfills at all.

Each update struct carries `symbol`, `observed_at`, `published_at`,
`connection_id: Option<String>`, `value: Decimal`, `raw: String`, and
`display_value: f64`. `TwapUpdate` additionally carries `window: TwapWindow` —
not an `Option`, because on that type it is statically always present.

**Why the two spot types are not merged:** `BinanceUpdate` and
`ChainlinkSpotUpdate` are field-identical and semantically different. Keeping
two structurally identical things apart is the one job a type system does that a
runtime `Scale` tag cannot — a tag does not distinguish them, it *records* a
distinction someone already had to make correctly elsewhere.

Snapshot points encode the observed asymmetry:

```rust
pub struct Snapshot {
    pub topic: Topic,
    pub symbol: String,
    pub published_at: i64,
    pub points: SnapshotPoints,
}

#[non_exhaustive]
pub enum SnapshotPoints {
    /// Binance and Chainlink spot. The server sends no `full_accuracy_value`
    /// in these backfills, so no exact value exists to expose.
    DisplayOnly(Vec<DisplayPoint>),  // { observed_at, display_value: f64 }
    /// Chainlink TWAP. Exact values present.
    Exact(Vec<ExactPoint>),          // { observed_at, value, raw_e18, display_value }
}
```

**Decoding.** E18 goes through `Decimal::try_from_i128_with_scale(v, 18)`,
never `from_i128_with_scale`, which panics on overflow — a price feed must not
be able to panic a caller's task. Overflow returns `RtdsError::Precision`.
Verified against `rust_decimal` 1.37 on 2026-09-05: the captured TWAP value
`79697474565615044788224` decodes to exactly `79697.474565615044788224`, and an
`i128` beyond `Decimal`'s range returns `Err` rather than panicking.
`Decimal`'s 96-bit mantissa (max ≈ 7.92 × 10²⁸) caps the representable price at
roughly $79.2 billion per unit after the E18 divide; the observed BTC value
(≈ 7.97 × 10²²) uses about three of eight orders of headroom, so the carrier is
right, but the guard is still required.

Every struct keeps `raw` alongside `value`. The decoded `Decimal` is polyoxide's
interpretation; the raw string is what the server said. Given six documented
facts turned out wrong here, retaining the unparsed original is what lets a
future disagreement be diagnosed rather than argued about.

### Connection tiers

Mirrors the existing `WebSocket` / `WebSocketWithPing` split.

**Tier 1 — `Rtds`.** `impl Stream<Item = Result<PriceEvent, RtdsError>>`.
Connects, sends the subscription frame, retains the subscriptions so tier 2 can
replay them, exposes `ping()` and `close()`. Ends on disconnect. Installs the
rustls `CryptoProvider` before connecting, using the same best-effort `Once` as
`polyoxide-clob/src/ws/client.rs:42`. The duplication is deliberate and safe:
`install_default` returns `Err` when one is already set, so two crates racing is
a no-op rather than a conflict. The copy carries a comment naming its twin.

**Tier 2 — `RtdsBuilder` → `SupervisedRtds`.**

```rust
RtdsBuilder::new()
    .ping_interval(Duration::from_secs(5))   // documented cadence; write-only
    .stale_after(Duration::from_secs(30))    // the actual liveness signal
    .backoff(Duration::from_millis(500), Duration::from_secs(60))
    .connect(subs).await?
    .run(|event| async move { Ok(()) }).await?;
```

**Staleness, not heartbeat, is the liveness signal.** There is no `PONG`, so a
half-open socket is indistinguishable from a quiet market except by timing the
gap between updates. `stale_after` is measured across *all* subscriptions on the
connection, not per symbol. Observed cadence is ~1 update/second per symbol per
topic, so a 30-second default is roughly 30× headroom.

**Reconnect must distinguish fatal from recoverable.** A dropped socket is
recoverable. `RtdsError::Server` — the frame an unrecognised topic produces — is
not: reconnecting replays the same rejected subscription forever, at full
backoff, silently. `RtdsError::is_recoverable()` gates the retry, following the
same classifier convention as `ApiError::is_retriable()`.

**Callers must expect repeated snapshots.** Because every resubscribe replays a
fresh backfill, a reconnect does not merely resume the feed — it re-initialises
caller state. Recovery is therefore *better* than the documentation promises,
and that is precisely why `Snapshot` is a first-class stream event rather than
something consumed internally at connect time.

### Errors

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RtdsError {
    Connection(Box<tungstenite::Error>),
    Json(serde_json::Error),
    ConnectionClosed,
    /// The `{"body":…,"statusCode":N}` envelope. Never recoverable.
    Server { status_code: u16, message: String },
    /// A value that will not fit a `Decimal` after scaling.
    Precision { raw: String, topic: Topic },
    /// No frame on any subscription within `stale_after`.
    Stalled { elapsed: Duration },
    Url(url::ParseError),
}

impl RtdsError {
    pub fn is_recoverable(&self) -> bool;  // false for Server, Precision, Json
}
```

## Testing

Fixtures are the frames captured verbatim on 2026-09-05 (appendix below), in a
`fixtures` module documented as `polyoxide-clob/src/ws/sports.rs:90` documents
its own: capture date, count, and one constant per distinct observed shape.

**The scale test must be differential.** `assert!(update.value > Decimal::ZERO)`
passes on both the Binance and Chainlink-spot frames, which is the shape-only
failure this repo has hit repeatedly. The test that holds the design up feeds
the two field-identical captured frames to their two types and asserts the
results differ by a factor of 10¹⁸. It fails the moment someone merges the types
or copies the wrong scale, and it cannot pass by accident.

Paired with it, a test asserting the Binance value is exactly `79697.73` and
naming `0.00000000000008` in its failure message as what the bug looks like, so
the regression is legible at the failure site.

Other tests that earn their place:

- The serialised filter is byte-exactly `{"symbol":"btc/usd"}`, no spaces.
- `.symbols([a, b])` produces two subscription entries, not one.
- The captured error frame parses to `RtdsError::Server { status_code: 401, .. }`
  with `is_recoverable() == false`.
- Spot snapshots deserialise to `SnapshotPoints::DisplayOnly`; TWAP snapshots to
  `SnapshotPoints::Exact`.
- An E18 value exceeding `Decimal`'s range returns `Precision`, not a panic.

**Supervision needs a scripted local server.** `mockito` is HTTP-only and there
is no WebSocket mocking anywhere in this workspace. Backoff, staleness detection
and resubscribe cannot be exercised deterministically against a live host, so
`tests/scripted_server.rs` provides a `tokio-tungstenite::accept_async` harness
(dev-dependency only) that can serve canned frames, fall silent on cue, and drop
a connection on cue. The tests assert the client resubscribes with the *same*
frames after a drop, and that `Stalled` fires when frames stop without a close.

## Deliverables

**Live tests and nightly.** `tests/live_api.rs` with `#[ignore]`, plus a matrix
entry `- { crate: polyoxide-rtds, flags: "--test live_api" }` in
`.github/workflows/nightly-behavioral.yml`.

The live test runs a **control subscription**. A silent feed is ambiguous: it
happens both when upstream is down and when our filter encoding regresses, and
the whitespace trap makes the second look exactly like the first. Classifying
silence as environmental would hide the most likely real failure permanently.
So the test subscribes filtered *and* unfiltered:

- both silent → environmental; panic with the phrase `legitimately time out`,
  which `ENVIRONMENTAL_RE` in `.github/scripts/classify_failures.py:37` already
  matches
- control receiving, filtered silent → a **real** failure, because that is the
  filter breaking

This requires **no change to `classify_failures.py`**, and follows the same
control-subscription discipline as `live_user_subscription_accepts_omitted_markets`.

**Spec mirror.** `docs/specs/rtds/asyncapi-live-data.json`, observed-only,
modelled on `docs/specs/clob/asyncapi-sports.json` with `x-observed-payload`,
since upstream publishes no AsyncAPI for this host.
`docs/specs/rtds/OBSERVED.md` records the six contradictions, the whitespace
trap, batch poisoning, and the `PING` finding, modelled on
`docs/specs/gamma/OBSERVED.md`. A row is added to `docs/specs/INDEX.md`.

**`nightly-schema.yml`: deliberately not added.** There is no published document
to diff against, so a drift job would alarm forever. Recorded alongside the
existing deliberate exclusions (the sports AsyncAPI mirror, `user-pnl-api`,
`lb-api`).

**CLI.** `polyoxide ws prices --topic <t> --symbol <s> --window <30|60> --json`,
joining the existing `ws` group rather than adding a top-level one.
`polyoxide-cli` gains a direct `polyoxide-rtds` dependency, consistent with it
depending on component crates rather than the unified crate.

**Unified crate.** An `rtds` feature on `polyoxide`, added to `full`, kept out of
`default` so the default build stays light.

**Release.** `polyoxide-rtds` added to the publish sequence in
`.github/workflows/release.yml`, after `polyoxide-core`.

**CLAUDE.md.** Crate graph, publish order, and the RTDS gotchas — specifically
the per-topic scale of `full_accuracy_value`, the whitespace trap, and batch
poisoning.

## Open questions

**Resolved 2026-09-05 by live probe: yes.**

The question was whether RTDS accepts a second `action:"subscribe"` frame on an
already-open connection. It does. A connection subscribed to
`crypto_prices_twap_thirty` for `btc/usd` was sent a second subscribe frame
adding `crypto_prices_chainlink`, and update frames from **both** topics
followed on the same socket within seconds. Repeated four times across two
sessions; no `{"body":…,"statusCode":…}` rejection was ever returned.

So `Rtds::subscribe_more()` exists and mirrors the CLOB user channel's
`subscribe_markets`: a caller can widen a subscription without reconnecting.
The live test `reports_whether_a_second_subscribe_frame_is_accepted` pins it,
so if the venue ever withdraws the behaviour, that shows up as a nightly
failure rather than as a method that silently does nothing.

## Appendix: captured frames

Captured 2026-09-05 from `wss://ws-live-data.polymarket.com`. Snapshot frames
are shown with their `data` arrays truncated to three points; the real lengths
are noted. `crypto_prices_chainlink|subscribe` was observed in a separate probe
(payload keys `data`, `symbol`; 57 points; point keys `timestamp`, `value` —
i.e. `DisplayOnly`) but no verbatim frame was captured in the final batch.

### The decisive pair — field-identical, different scale

```json
{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697.73000000","symbol":"btcusdt","timestamp":1788600389000,"value":79697.73},"timestamp":1788600389154,"topic":"crypto_prices","type":"update"}
```

```json
{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79696948174287960000000","symbol":"btc/usd","timestamp":1788600388000,"value":79696.94817428796},"timestamp":1788600389451,"topic":"crypto_prices_chainlink","type":"update"}
```

Both report BTC at ≈ $79,697, roughly one second apart, with identical payload
keys. The first is a plain decimal; the second is E18.

### TWAP updates

```json
{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697474565615044788224","symbol":"btc/usd","timestamp":1788600388000,"value":79697.47456561505,"window_s":30},"timestamp":1788600389537,"topic":"crypto_prices_twap_thirty","type":"update"}
```

```json
{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697575317474428059648","symbol":"btc/usd","timestamp":1788600388000,"value":79697.57531747443,"window_s":60},"timestamp":1788600389495,"topic":"crypto_prices_twap_sixty","type":"update"}
```

### Snapshots

TWAP 30s, truncated from 55 points — note `full_accuracy_value` present and
`window_s` on the payload:

```json
{"payload":{"data":[{"full_accuracy_value":"79696840994573453885440","timestamp":1788600329000,"value":79696.84099457346},{"full_accuracy_value":"79696885010084155883520","timestamp":1788600330000,"value":79696.88501008415},{"full_accuracy_value":"79696928978311781023744","timestamp":1788600331000,"value":79696.92897831179}],"symbol":"btc/usd","window_s":30},"timestamp":1788600388753,"topic":"crypto_prices_twap_thirty","type":"subscribe"}
```

Binance, truncated from 120 points — note **no** `full_accuracy_value` on the
points and **no** `window_s`, and no `connection_id` on the envelope:

```json
{"payload":{"data":[{"timestamp":1788600269000,"value":79697.73},{"timestamp":1788600270000,"value":79697.73},{"timestamp":1788600271000,"value":79697.73}],"symbol":"btcusdt"},"timestamp":1788600388752,"topic":"crypto_prices","type":"subscribe"}
```

### Error frame

Produced by including one unrecognised topic in an otherwise valid five-topic
batch. All five topics returned zero frames.

```json
{"body":{"message":"leger GetTopics error: rpc error: code = NotFound desc = topic: definitely_not_a_topic and type: update not found, status: rpc error: code = NotFound desc = topic: definitely_not_a_topic and type: update not found, message: topic: definitely_not_a_topic and type: update not found"},"statusCode":401}
```
