---
paths:
  - "polyoxide-polymarket/src/clob/ws/**"
  - "polyoxide-polymarket/tests/{live_clob_ws,live_clob_trading,trading_clob}.rs"
  - "polyoxide-polymarket/examples/ws_{market,user}.rs"
  - "polyoxide-cli/src/commands/polymarket/clob/stream/**"
  - "docs/specs/clob/asyncapi-*.json"
---

# Polymarket CLOB sockets

Feature `clob-ws` (not on by default; the crate's default is `clob`, `gamma` and `data`). The
contracts are AsyncAPI, mirrored in `docs/specs/clob/asyncapi-{market,user}.json`. Generic socket
rules: `sockets.md`.

## The bare tier

- Two channels: `WebSocket::connect_market(asset_ids)` (public) and
  `WebSocket::connect_user(condition_ids, credentials)` (authenticated). Implements
  `futures_util::Stream`; `WebSocketBuilder` adds auto-ping keep-alive for long-running
  connections, and accepts only `wss://`.
- Its error is `ClobWsError`, whose never-constructed `ConnectionClosed` variant stays until S3,
  because a protected test builds it. `Authentication` is built by the supervised user socket.
- Three market events, `best_bid_ask`, `new_market` and `market_resolved`, are withheld by the
  server unless the subscription sets `custom_feature_enabled`:
  `WebSocket::connect_market_with(ids, MarketSubscriptionOptions::default().with_custom_features())`.
  `MarketMessage` and `Channel` are `#[non_exhaustive]`, since upstream adds event types over time.
- `MarketMessage::from_json` keeps the first book of a frame that carries several, which a
  protected test pins.
- The user channel's market filter is optional: `WebSocket::connect_user_all_markets(creds)` omits
  it and receives events for every market, and `subscribe_markets`/`unsubscribe_markets` adjust it
  on a live connection without reconnecting. The market channel changes membership the same way,
  with `subscribe_assets`/`unsubscribe_assets`, or a `MembershipHandle` (from
  `WebSocketWithPing::membership`, taken **before** `run`) while the ping loop drives the socket.
  Both frames are documented in the mirrors (`SubscriptionRequestUpdate`).
- Verified live 2026-09-09: an added asset gets a fresh `book` in ~155 ms; a duplicate add gets
  nothing (unsubscribe then subscribe to force a snapshot); an empty-membership socket stays open
  under the 10 s `PING` but is reset after ~125 s without it.

## The supervised market socket

`clob::ws::MarketWsBuilder` and its `SupervisedMarketWs`, in `src/clob/ws/supervised/`;
`ClobMarketProtocol` describes the market channel to the kit's `Supervisor`.

- The stream yields `MarketEvent`s: the channel's messages, and one `Disconnected` and one
  `Reconnected` per outage. A `Disconnected` carries a `DisconnectReason`, the kit's `Outage` as
  `Closed`, `Error`, `Stale` or `Rotation`.
- It reconnects after a server close, a stall or a lost connection, and replays the whole
  membership, changes and the builder's `MarketSubscriptionOptions` included, as one subscribe
  frame on every connection, an empty one too, which the venue keeps open.
- It sends the text `PING` every 10 s on the wall clock (D4), the first one interval after the
  connection opens; counts every inbound frame as liveness and is stale after 30 s (D5); backs off
  500 ms to 60 s with the kit's `Retry-After` floor; never rotates; queues 1024 events.
- `MarketWsBuilder::connect(asset_ids)` refuses more than 500 ids before connecting and returns
  the first attempt's failure instead of retrying it.
- A frame that carries several books yields every one. `PONG`, `{}`, empty text and any text
  without an `event_type` are skipped, and a Binary frame is decoded as text (D12).
- A frame that does not decode is yielded as `ClobWsError::Json` and the connection stays;
  `ClobWsError::Authentication` stops the stream; a connection used after it closed reconnects
  (D19), though its class is `InvalidRequest`; any other failure reconnects exactly when its class
  is retriable, with no match over `tungstenite::Error`.
- The connection is dialled by clob's per-address `dial` through `Protocol::connect` (D20), so
  every address of a blackholed route is tried within `connect_timeout` per step under supervision
  too.
- The URL is `wss://`, or `ws://` to a loopback host, which `MarketWsBuilder::url` enforces.
- A change through `MarketMembership` (on the kit's `MembershipHandle<ClobMarketProtocol>`) is
  recorded before its frame is sent and sends the ids that change, once each. One whose connection
  dies under it is answered `Ok` and replayed; one made during an outage is answered at once and
  replayed; a subscribe past 500 ids is refused as `InvalidMessage` and changes nothing.
- `clob::ws::scripted`, built for the unit tests, holds the `PING`/`PONG` answerer both
  supervised sockets' tests share.

## The supervised user socket

`UserWsBuilder` and `SupervisedUserWs` in `clob::ws::supervised`, beside the market socket and on
the same kit `Supervisor`, with `ClobUserProtocol`, `UserEvent`, `UserChange` and
`UserMembership`, all at `clob::ws`.

- The membership is `Option<Vec<String>>` (AD-11): `None` follows every market and the subscribe
  frame omits `markets`; `Some(markets)` carries them; `Some(vec![])` sends `"markets": []`, which
  follows none. Nothing ever turns a `Some` into `None`, so unsubscribing the last market and then
  reconnecting replays an empty filter and never widens the subscription to every market (a test
  pins it).
- A subscribe on `None` changes and sends nothing; an unsubscribe on `None` is refused as
  `InvalidMessage`; on a `Some`, each change sends one update frame for the markets it actually
  changed, at most 500 per connection. A change is recorded before its frame is sent, and one made
  during an outage is answered at once, so the replay carries both.
- The protocol holds the account's `ApiCredentials` and rebuilds the credential-bearing
  `UserSubscription` on every connection, so a reconnect replays them. The builder holds none:
  `connect` and `start` take them (`start` begins in an outage when the first attempt fails in a
  way recovery would retry).
- The queue is `Queue::Unbounded` by the owner's choice (AD-24): the kit never waits on the
  consumer, and a consumer that stops reading loses no fill, at the cost of memory that grows with
  its backlog.
- A server close with code 1008 is how the host refuses credentials (about 1.2 s after the
  subscribe, with no error frame; probed live 2026-10-17). It decodes as
  `ClobWsError::Authentication(<the close reason>)`, classed `Unauthorized`, and recovery stops on
  it, so the stream yields the error and ends instead of reconnecting forever at the backoff
  ceiling.
- Every `UserEvent::Disconnected` is followed by one `UserEvent::Reconnected`, unless the stream
  ends on an error recovery stops on (a reconnect refused with a status retrying cannot fix, such
  as 401, 403 or 404), which it yields first. What the socket carried in between is lost, so a
  consumer reconciles there.
- A frame that does not decode is yielded as an error and skipped; a JSON array yields one event
  per element, and an array with an element that does not decode is skipped whole.
- The URL must be `wss://`, or `ws://` to a loopback host, so credentials never travel unencrypted
  to a remote host.
- Its suites drive the kit's scripted server through `src/clob/ws/scripted.rs`'s `PING`/`PONG`
  answerer.
