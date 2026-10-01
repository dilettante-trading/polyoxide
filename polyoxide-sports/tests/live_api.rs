//! Live tests against wss://sports-api.polymarket.com/ws. Ignored by default:
//!
//! ```text
//! cargo test -p polyoxide-sports --test live_api -- --ignored --nocapture
//! ```
//!
//! The feed carries only matches that are live somewhere. A test that waits
//! for a frame says so when it times out, in the words the nightly
//! classifier treats as environmental.
//!
//! `nightly-schema.yml` excludes this host, because the published AsyncAPI
//! document does not match the wire. So
//! `live_frames_round_trip_and_carry_no_unmodelled_keys` is this host's drift
//! detector: when upstream adds a field, it fails and names the key.

use std::{collections::BTreeMap, time::Duration};

use futures_util::StreamExt;
use polyoxide_sports::{Event, MatchUpdate, SportsWs, SportsWsBuilder, SPORTS_WS_URL};
use serde_json::Value;
use tokio::time::{timeout, timeout_at, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const RECV_WINDOW: Duration = Duration::from_secs(45);
const QUIET: &str = "if no matches are live anywhere this can legitimately time out, \
                     so re-run before concluding a defect";

/// The raw socket tests call `connect_async` directly, so they install the
/// provider the crate would. See `ensure_crypto_provider` in src/client.rs.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[tokio::test]
#[ignore]
async fn live_bare_feed_yields_a_parsed_frame() {
    let mut feed = SportsWs::connect()
        .await
        .expect("connect to the sports feed");
    let update = timeout(RECV_WINDOW, feed.next())
        .await
        .unwrap_or_else(|_| panic!("no frame within {RECV_WINDOW:?}; {QUIET}"))
        .expect("the stream ended instead of yielding a frame")
        .expect("the frame parses");
    assert!(!update.league_abbreviation.is_empty(), "{update:?}");
    assert!(
        update.key().is_some(),
        "the frame identifies no match: {update:?}"
    );
    println!("first frame: {update:?}");
}

/// Upstream documents a text "ping"/"pong" exchange that a client must
/// answer within 10 seconds. The server actually sends protocol pings that
/// the transport answers. If upstream were right, a client that never sends
/// a text "pong" would be dropped well inside this window.
#[tokio::test]
#[ignore]
async fn live_bare_connection_survives_the_keepalive_interval() {
    let mut feed = SportsWs::connect()
        .await
        .expect("connect to the sports feed");
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut frames = 0usize;
    loop {
        match timeout_at(deadline, feed.next()).await {
            Err(_) => break,
            Ok(Some(Ok(_))) => frames += 1,
            Ok(Some(Err(e))) => panic!("the stream failed after {frames} frames: {e}"),
            Ok(None) => panic!(
                "the server closed the connection after {frames} frames; the keep-alive \
                 is not being answered"
            ),
        }
    }
    println!("survived 40 s with {frames} frames");
}

/// The 45-second staleness default rests on this cadence.
#[tokio::test]
#[ignore]
async fn live_server_sends_protocol_pings_every_15_seconds() {
    install_crypto_provider();
    let (mut socket, _) = connect_async(SPORTS_WS_URL).await.expect("connect");
    let started = Instant::now();
    let deadline = started + Duration::from_secs(40);
    let mut pings = Vec::new();
    loop {
        match timeout_at(deadline, socket.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Message::Ping(_)))) => pings.push(started.elapsed()),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => panic!("the socket failed: {e}"),
            Ok(None) => panic!("the server closed the socket"),
        }
    }
    assert!(
        pings.len() >= 2,
        "expected at least two protocol pings in 40 s, saw {pings:?}; the staleness limit depends on them"
    );
    let gaps: Vec<Duration> = pings.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        gaps.iter().all(|gap| *gap < Duration::from_secs(20)),
        "ping gaps {gaps:?} exceed 20 s; revisit the 45 s staleness default"
    );
}

/// Held past the 45-second staleness limit with default settings, the
/// supervised feed must not disconnect.
#[tokio::test]
#[ignore]
async fn live_supervised_feed_holds_past_the_stale_limit() {
    let mut feed = SportsWsBuilder::new()
        .connect()
        .await
        .expect("connect to the sports feed");
    let deadline = Instant::now() + Duration::from_secs(50);
    let mut updates = 0usize;
    loop {
        match timeout_at(deadline, feed.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Event::Update(_)))) => updates += 1,
            Ok(Some(Ok(Event::Disconnected { reason }))) => {
                panic!("disconnected after {updates} updates: {reason}")
            }
            Ok(Some(Ok(other))) => panic!("unexpected event {other:?}"),
            Ok(Some(Err(e))) => panic!("a live frame did not parse: {e}"),
            Ok(None) => panic!("a supervised feed never ends"),
        }
    }
    println!("held 50 s with {updates} updates");
}

#[tokio::test]
#[ignore]
async fn live_frames_round_trip_and_carry_no_unmodelled_keys() {
    install_crypto_provider();
    let (mut socket, _) = connect_async(SPORTS_WS_URL).await.expect("connect");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut checked = 0usize;
    // Each unmodelled key, with a league it was seen on.
    let mut unmodelled: BTreeMap<String, String> = BTreeMap::new();
    while checked < 100 {
        let message = match timeout_at(deadline, socket.next()).await {
            Err(_) => break,
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(e))) => panic!("the socket failed: {e}"),
            Ok(None) => panic!("the server closed the socket"),
        };
        let Message::Text(text) = message else {
            continue;
        };
        let update = MatchUpdate::from_json(&text)
            .unwrap_or_else(|e| panic!("a live frame did not parse: {e}\n{}", text.as_str()));
        let original: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&update).unwrap(),
            original,
            "a live frame changed on a round trip"
        );
        for key in update.extra.keys() {
            unmodelled
                .entry(key.clone())
                .or_insert_with(|| update.league_abbreviation.clone());
        }
        checked += 1;
    }
    assert!(checked > 0, "no frames within 60 s; {QUIET}");
    assert!(
        unmodelled.is_empty(),
        "the feed sends keys MatchUpdate does not model (key: league seen on): {unmodelled:?}. \
         Model them in MatchUpdate, refresh fixtures with scripts/capture_sports_fixtures.py, \
         and record them in docs/specs/sports/OBSERVED.md"
    );
    println!("checked {checked} frames");
}
