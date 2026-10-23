---
paths:
  - "polyoxide-kalshi/**"
  - "scripts/capture_kalshi_*.py"
  - "docs/specs/kalshi/**"
---

# Kalshi

`polyoxide-kalshi`'s `predictions` module (feature `predictions`, on by default; its streams under
`predictions-ws`) reads Kalshi's event-contract market data: `Predictions::builder()`, namespaces
`kalshi.health()`, `exchange()`, `markets()`, `events()`, `series()`, `historical()`,
`account()`. The umbrella re-exports it as `polyoxide::kalshi`. Kalshi's OpenAPI and AsyncAPI are
mirrored in `docs/specs/kalshi/`, and `OBSERVED.md` records where its hosts part ways with them.

## Credentials

- `polyoxide_kalshi::predictions::Credentials`: an API key id and an Ed25519 or RSA private key,
  read once from PEM, passed through `PredictionsBuilder::credentials`. No client reads them from
  the environment; a client built without them sends unsigned requests.
- `live_predictions_account` and `live_predictions_ws` need them and run against the demo host,
  reading `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text) through
  test-support's loaders; they are auth-gated without them. Keys do not cross environments, and
  production credentials never reach CI. `live_predictions` and `live_predictions_venue` read
  production unsigned and need no secrets.

## Requests

- The Read throttle (`src/predictions/throttle.rs`): `rate-limits.md`.
- `tickers` on `ListMarkets`, `ListMarketWindow` and `ListEvents` is hand-written: it refuses an
  empty set, or one past `TICKERS_QUERY_BUDGET` (8,000 bytes, half the request target Kalshi
  answers `414` past), before anything is sent.
- Its `From<ApiError>` turns an unsuccessful response with one of Kalshi's three body shapes into
  `Venue` (`errors.md`).

## `predictions::venue`

`MarketData`, `Trades`, `EventGroups` and `Candles` on `Predictions`, with `InstrumentExt`,
`TradeExt`, `EventGroupExt` and `CandleExt`.

- `Candles::intervals()` is one minute, one hour and one day (`period_interval` 1, 60 and 1440),
  and any other width is the `UnservedInterval` refusal before a request: the batch candlestick
  route answers an unserved width with a `200` and the daily candles, so an unserved width would
  silently read as daily, and the refusal is the only guard.
- The route also leaves out whole periods, so a span can return fewer candles than it has periods,
  and a query is clamped to 10,000 periods.
- `candles` read `GET /markets/candlesticks` for the one ticker, asking for the query's span moved
  on by one width, because Kalshi selects a candle by its end and the trait by its open, and put
  each candle's `open_time` one width before its end. A `no` key mirrors every price (1 - p, with
  high and low swapped). A period with no trade has only a `close` (the venue's previous price) and
  is skipped without one. `CandleExt` holds the YES bid and ask as `Ohlc`, `open_interest` and
  `mean`.
- A market settled before Kalshi's historical cutoff is gone from the live routes and is read from
  `historical()` instead (#71): `instrument` when the live market route answers `404`; `candles`
  when the batch answer leaves the market out (clamped to 5,000 periods, the archive's cap);
  `trades` when the live route has none; `event_group` when the live event does not nest the
  key's market. The archive's `404` is `NotListed`, and no method reads the cutoff.

## The socket (`predictions-ws`)

`PredictionsWsBuilder`, `SupervisedPredictionsWs` and `PredictionsProtocol`, on the kit's
`Supervisor`.

- **Channels.** It subscribes `orderbook_delta` with `use_yes_price: true`, so a snapshot's asks
  are the NO bids in YES pricing and one ladder serves both outcomes; and, when
  `PredictionsWsBuilder::channels` asks for them, `ticker` and `trade` as well (a `Channel`,
  deduplicated, `OrderbookDelta` by default; an empty set is `PredictionsWsError::InvalidRequest`
  at `connect`). One `subscribe` names every channel and the tickers, each channel's `subscribed`
  answer gives it its own `sid`, and the stream yields `Event::Ticker` and `Event::Trade` beside
  the snapshots and deltas.
- **A signed handshake.** Kalshi refuses a handshake without its API-key headers, public data
  included, so `connect` takes `Credentials` and the handshake is signed afresh for every attempt,
  the first and each reconnect: `KALSHI-ACCESS-KEY`, `KALSHI-ACCESS-TIMESTAMP` and
  `KALSHI-ACCESS-SIGNATURE` over `timestamp_ms + "GET" + the URL's path`, by the signer REST uses.
  The kit's scripted server records each handshake's headers, which is how the suites prove it.
  The URL must be `wss://`, or `ws://` to a loopback host, so a signed handshake never travels
  unencrypted to a remote host.
- **Membership** is a set of tickers, replayed as one `subscribe` on every connection and changed
  with one `update_subscription` per `sid`, in channel order. The first add sends a `subscribe`,
  emptying it sends an `unsubscribe` for every `sid`, and an empty set never subscribes, since
  Kalshi refuses a `ticker` or `trade` subscribe with no markets (code 14 on the demo host,
  2026-10-21; a venue's empty membership must not subscribe, A9-4).
- **Changes.** A change is recorded before its frame is sent: one whose connection dies under it
  is answered `Ok` and replayed; one made during an outage is answered at once and replayed; one
  Kalshi refuses is undone. A refusal of the first command fails the change and undoes it on a
  kept connection. A refusal after a command of the same change was accepted also fails and undoes
  it, and ends the connection, so the replay brings the membership and Kalshi's subscriptions back
  into agreement. An error of code 9 stops the stream, wherever it arrives.
- **Sequence.** Every frame that carries `sid` and `seq`, the answers included, is checked per
  `sid`. A gap on the `trade` `sid` is `PredictionsWsError::SequenceGap` and reconnects, since that
  channel has no snapshot (the `ticker` channel carries no `seq`). A gap on the order-book `sid`
  re-snapshots in place: the stream yields `Event::Resyncing { tickers }` (the whole membership),
  drops the frame and every `orderbook_delta` on that `sid` until each ticker's snapshot is in, and
  queues through the kit's `Protocol::follow_up` (A9-3) one `update_subscription` with
  `action: "get_snapshot"` for every ticker. Each snapshot that answers re-anchors the `sid`'s
  sequence, and a frame that carries the follow-up's id is a push, never the answer to a
  membership command. The stream reconnects instead (`PredictionsWsError::SnapshotTimeout`,
  `Unavailable`) when the snapshots are not all in by `stale_after`, on an error frame answering
  the follow-up, and on a second gap during the resync.
- **Snapshots on demand.** `PredictionsMembership::snapshot(tickers)` asks for the same snapshots
  (`Change::Snapshot`): answered `Ok` once the frame is sent, since Kalshi acknowledges no
  `get_snapshot`; at once and unrecorded during an outage, since the replay brings a fresh snapshot
  of every ticker; and `PredictionsWsError::InvalidRequest` for a ticker outside the membership or
  without the order-book channel.
- **The exchange clock.** `OrderbookSnapshot::clock` and `OrderbookDelta::clock` carry the
  `clock` the AsyncAPI has required since #72 (2026-10-22): the same on every connection, so a
  consumer with redundant connections can match a market's updates by it. They are optional,
  since no capture carries the key yet (`ABSENT_ON_THE_WIRE` in
  `wire_agreement_predictions_ws.rs`). The socket finds a gap by `seq` alone, and `Books` does
  not read the clock.
- The kit's `request` consumes an answer before `decode` sees it, so the request's correlation
  closure counts the answers and the pushes it reads while it waits, and `decode` counts a push
  the closure kept only once; a gap an answer showed is raised by the next frame.
- It pings every 10 s on the wall clock and counts every frame as liveness, the server's Pings
  included.
- **Recovery** follows the error's class (a refused handshake's 401 or 403 stops the stream; a 429
  or 5xx retries after at least its `Retry-After`, through `Protocol::retry_after`), except:
  - a frame that does not decode is skipped, and a connection used after it closed reconnects;
  - an error frame's code decides: 9 stops the stream; 10, 17, 18, 25 and 27 reconnect (Kalshi's
    AsyncAPI calls 10 and 25 terminal, "the user must resubscribe", which the replay does, and 17
    and 18 are the venue's own failures, all `Unavailable`); any other is yielded and skipped;
  - except when it refuses the replay's `subscribe`, which ends the stream with that error, as does
    an answer to that `subscribe` that does not decode (`Json`). A change made during an outage, or
    whose connection died under it, was answered `Ok`, so the replay is where its caller learns of
    the refusal, and one refused ticker ends the stream for every ticker. (`recover` reads a flag
    `replay` sets just before it returns Kalshi's error or the decode error, which `reset` clears
    for the next attempt, since the kit reads a skipped error on a failed attempt as a reconnect,
    and an answer that never decodes would reconnect forever. A frame that does not decode
    elsewhere stays yielded and skipped.)
- **`ws::book::Books`** is a value the consumer feeds events into: `Books::apply(&event)` returns
  the `OrderBook` the event changed, and `Books::get(ticker)` reads one. It keeps each ticker's
  ladder in YES pricing from `Snapshot` and `Delta` (a delta adds its signed size to its level and
  removes the level at zero or below; a delta for a ticker with no snapshot is ignored), and drops
  the books `Resyncing` names and every book on `Reconnected`. Each `OrderBook` gives `best_bid`,
  `best_ask`, `bids` (highest first), `asks` (lowest first) and `levels(outcome)`, where a NO bid at
  *q* is a YES ask at *1 - q*. It does no I/O and has no clock.
