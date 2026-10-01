# polyoxide-sports

Rust client for Polymarket's live sports feed at
`wss://sports-api.polymarket.com/ws`: live scores for every match in every
league, with no credentials and no subscription.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-sports/).

## Installation

```toml
[dependencies]
polyoxide-sports = "0.36"
```

## Usage

`SportsWsBuilder` builds a feed that reconnects on its own and says when
scores may be stale:

```no_run
use futures_util::StreamExt;
use polyoxide_sports::{Event, SportsWsBuilder};

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
        // Only a frame this crate could not read arrives as an error, and
        // the feed carries on after it.
        Err(error) => eprintln!("skipped a frame: {error}"),
    }
}
# Ok(())
# }
```

`SportsWs` is the bare stream underneath. It yields `MatchUpdate`s and ends
when its connection does.

## What the feed sends

- **Full state, often repeated.** Each frame is a match's whole current
  state. The server re-sends unchanged state on a timer, so roughly half of
  all frames repeat the previous one for their game. `MatchUpdate`
  implements `PartialEq`: compare against the last frame per `GameKey` to
  keep only changes. Frames carrying `eventState` may defeat this, if its
  timestamps change on every send; see the `MatchUpdate` docs.
- **The ended frame is sent once.** A feed that is disconnected when a
  match ends never sees it. After `Event::Reconnected`, look up games that
  may have ended through gamma's events list, which filters on `game_id`.
  Cricket uses a string id that gamma does not accept.
- **Two kinds of id.** Most sports send a numeric `gameId`; cricket sends a
  string `metadataGameId` instead. `MatchUpdate::key` returns a `GameKey`
  that covers both.
- **Protocol pings every 15 seconds.** The transport answers them. When
  nothing is live anywhere they are the only sign of life, so the supervised
  feed counts them toward its 45-second staleness limit.

Polymarket's published AsyncAPI document for this host describes a payload
and a text keep-alive that the server does not use. This crate is modelled
on captured frames instead.
