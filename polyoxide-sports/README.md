# polyoxide-sports

Rust client for Polymarket's live sports feed at
`wss://sports-api.polymarket.com/ws`: live scores for every match in every
league, with no credentials and no subscription.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-sports/).

## Installation

```toml
[dependencies]
polyoxide-sports = "0.37"
```

## Usage

`SportsWsBuilder` builds a feed that reconnects on its own and says when
scores may be stale:

```no_run
use futures_util::StreamExt;
use polyoxide_sports::{Event, SportsError, SportsWsBuilder};

# async fn run() -> Result<(), polyoxide_sports::SportsError> {
let mut feed = SportsWsBuilder::new().connect().await?;
while let Some(event) = feed.next().await {
    match event {
        Ok(Event::Update(update)) => {
            println!("{} {} {}", update.league_abbreviation, update.score, update.period)
        }
        Ok(Event::Disconnected { reason }) => eprintln!("scores are stale: {reason}"),
        Ok(Event::Reconnected) => eprintln!("reconnected"),
        Ok(_) => {}
        // A frame this crate could not read; the feed carries on.
        Err(SportsError::Decode { source, .. }) => eprintln!("skipped a frame: {source}"),
        // A reconnect refused for good, such as a 404. The feed ends.
        Err(error) => return Err(error),
    }
}
# Ok(())
# }
```

`SportsWs` is a bare stream for callers who handle reconnects themselves. It
yields `MatchUpdate`s and ends when its connection does.

## Migrating from `polyoxide-clob`

Until 0.36.0 the sports feed was a channel of `polyoxide-clob`'s `ws` module.
It now lives here, and clob no longer has it.

| Before, in `polyoxide_clob::ws` | Now, in `polyoxide_sports` |
|---|---|
| `WebSocket::connect_sports()` | `SportsWs::connect()`, or `SportsWsBuilder::new().connect()` to reconnect on its own |
| `Channel::Sports(SportsMessage::Update(update))` | `MatchUpdate` from `SportsWs`; `Event::Update(Box<MatchUpdate>)` from the supervised stream |
| `SportsUpdateMessage` | `MatchUpdate`, with the same field names |
| `SportsMessage::from_json` | `MatchUpdate::from_json` |
| `WS_SPORTS_URL` | `SPORTS_WS_URL` |
| `WebSocketError` | `SportsError` |

Details that can affect existing code:

- `MatchUpdate` is `#[non_exhaustive]`, so it cannot be built with a struct
  literal; parse it with `MatchUpdate::from_json`.
- `extra` is a `serde_json::Map`, not a `serde_json::Value`.
- Serialising a `MatchUpdate` omits absent optional fields rather than
  writing `null`.
- In the unified `polyoxide` crate, enable the `sports` feature. The prelude
  exports `SportsWs`, `SportsWsBuilder`, `SupervisedSportsWs`, `SportsError`,
  and the aliases `SportsEvent`, `SportsMatchUpdate` and `SportsGameKey`.

## What the feed sends

- **Full state, often repeated.** Each frame is a match's whole current
  state. The server re-sends unchanged state on a timer: in one five-minute
  capture, 56 of 121 frames repeated the previous one for their game. `MatchUpdate`
  implements `PartialEq`: compare against the last frame per `GameKey` to
  keep only changes. Frames carrying `eventState` may defeat this, if its
  timestamps change on every send; see the `MatchUpdate` docs.
- **The ended frame is usually sent once.** A feed that is disconnected when a
  match ends never sees it. After `Event::Reconnected`, look up games that
  may have ended through gamma's events list, which filters on `game_id`.
  Cricket uses a string id that gamma does not accept.
- **Two kinds of id.** Most sports send a numeric `gameId`; cricket sends a
  string `metadataGameId` instead. `MatchUpdate::key` returns a `GameKey`
  that covers both.
- **Protocol pings every 15 seconds.** The transport answers them. When
  nothing is live anywhere they are the only sign of life, so the supervised
  feed counts them toward its 45-second staleness limit.
  Pongs go out only while the stream is polled, so keep slow work out of
  the loop: a long pause gets the connection dropped, which shows up as a
  disconnect.

Polymarket's published AsyncAPI document for this host describes a payload
and a text keep-alive that the server does not use. This crate is modelled
on captured frames instead.
