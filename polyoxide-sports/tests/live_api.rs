//! Live tests against wss://sports-api.polymarket.com/ws. Ignored by default:
//!
//! ```text
//! cargo test -p polyoxide-sports --test live_api -- --ignored --nocapture
//! ```
//!
//! The feed carries only matches that are live somewhere. A test that waits
//! for a frame says so when it times out, in the words the nightly
//! classifier treats as environmental. Only a window with no data frame of
//! any kind earns those words: binary frames, which this crate does not
//! read, fail as a real fault.
//!
//! `nightly-schema.yml` excludes this host, because the published AsyncAPI
//! document does not match the wire. So
//! `live_frames_round_trip_and_carry_no_unmodelled_keys` is this host's drift
//! detector. It sees only what is live while it runs, so the 06:00 UTC
//! nightly misses most North American leagues and weekend soccer; a capture
//! in a busy window covers those.

use std::{collections::BTreeMap, time::Duration};

use futures_util::StreamExt;
use polyoxide_sports::{Event, MatchUpdate, SportsError, SportsWs, SportsWsBuilder, SPORTS_WS_URL};
use serde_json::Value;
use tokio::{
    net::TcpStream,
    time::{timeout, timeout_at, Instant},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::CloseFrame, Message},
    MaybeTlsStream, WebSocketStream,
};

const RECV_WINDOW: Duration = Duration::from_secs(45);
/// How long the wire-agreement test reads: long enough for tennis's 30 to
/// 90 s rebroadcast to come round at least twice.
const WIRE_WINDOW: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const QUIET: &str = "if no matches are live anywhere this can legitimately time out, \
                     so re-run before concluding a defect";

type RawSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Fail a connect in words the nightly classifier reads correctly: a timeout
/// is transient, and the Debug form of anything else names its cause.
fn connect_failed<T>(error: SportsError) -> T {
    match error {
        SportsError::ConnectTimeout { after } => {
            panic!("the connect operation timed out after {after:?}")
        }
        other => panic!("could not connect to the sports feed: {other:?}"),
    }
}

/// Open a raw socket, bounded like the crate's own connects. These tests call
/// `connect_async` directly, so they install the TLS provider the crate
/// would; see `ensure_crypto_provider` in src/client.rs.
async fn connect_raw() -> RawSocket {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let connected = match timeout(CONNECT_TIMEOUT, connect_async(SPORTS_WS_URL)).await {
        Ok(connected) => connected,
        Err(_) => panic!("the connect operation timed out after {CONNECT_TIMEOUT:?}"),
    };
    let (socket, _) =
        connected.unwrap_or_else(|e| panic!("could not connect to the sports feed: {e:?}"));
    socket
}

/// What a close frame said, for a failure message.
fn describe_close(frame: Option<CloseFrame>) -> String {
    match frame {
        Some(frame) => format!(
            "code {}, reason {:?}",
            u16::from(frame.code),
            frame.reason.as_str()
        ),
        None => "a close frame with no code".to_owned(),
    }
}

#[tokio::test]
#[ignore]
async fn live_bare_feed_yields_a_parsed_frame() {
    let mut feed = SportsWs::connect().await.unwrap_or_else(connect_failed);
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
    let mut feed = SportsWs::connect().await.unwrap_or_else(connect_failed);
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut frames = 0usize;
    loop {
        match timeout_at(deadline, feed.next()).await {
            Err(_) => break,
            Ok(Some(Ok(_))) => frames += 1,
            Ok(Some(Err(SportsError::Decode { raw, source }))) => {
                panic!("a frame did not parse after {frames} frames ({source}): {raw}")
            }
            Ok(Some(Err(e))) => panic!("the connection failed after {frames} frames: {e}"),
            Ok(None) => panic!(
                "the server ended the connection after {frames} frames, inside 40 s. An \
                 unanswered keep-alive would do this, and so would a server restart, so \
                 re-run before concluding which"
            ),
        }
    }
    println!("survived 40 s with {frames} frames");
}

/// The 45-second staleness default rests on this cadence. The assertion
/// allows up to 20 s between pings.
#[tokio::test]
#[ignore]
async fn live_server_sends_protocol_pings_every_15_seconds() {
    let mut socket = connect_raw().await;
    let started = Instant::now();
    let deadline = started + Duration::from_secs(40);
    let mut pings = Vec::new();
    loop {
        match timeout_at(deadline, socket.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Message::Ping(_)))) => pings.push(started.elapsed()),
            Ok(Some(Ok(Message::Close(frame)))) => {
                panic!("the server closed the socket: {}", describe_close(frame))
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => panic!("the socket failed: {e}"),
            Ok(None) => panic!("the socket ended without a close frame"),
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
/// supervised feed must not disconnect. In a busy hour data frames alone keep
/// the staleness timer fed, so this proves ping-only liveness only when
/// updates are sparse. The printed longest gap between updates says which
/// kind of run it was.
#[tokio::test]
#[ignore]
async fn live_supervised_feed_holds_past_the_stale_limit() {
    let mut feed = SportsWsBuilder::new()
        .connect()
        .await
        .unwrap_or_else(connect_failed);
    let deadline = Instant::now() + Duration::from_secs(50);
    let mut updates = 0usize;
    let mut last = Instant::now();
    let mut longest = Duration::ZERO;
    loop {
        match timeout_at(deadline, feed.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Event::Update(_)))) => {
                updates += 1;
                longest = longest.max(last.elapsed());
                last = Instant::now();
            }
            Ok(Some(Ok(Event::Disconnected { reason }))) => {
                panic!("disconnected after {updates} updates: {reason}")
            }
            Ok(Some(Ok(other))) => panic!("unexpected event {other:?}"),
            Ok(Some(Err(SportsError::Decode { raw, source }))) => {
                panic!("a live frame did not parse ({source}): {raw}")
            }
            Ok(Some(Err(e))) => panic!("unexpected error: {e}"),
            Ok(None) => panic!("a supervised feed never ends"),
        }
    }
    longest = longest.max(last.elapsed());
    println!("held 50 s with {updates} updates; longest gap between updates {longest:?}");
}

#[tokio::test]
#[ignore]
async fn live_frames_round_trip_and_carry_no_unmodelled_keys() {
    let mut socket = connect_raw().await;
    let deadline = Instant::now() + WIRE_WINDOW;
    let mut leagues: BTreeMap<String, usize> = BTreeMap::new();
    let mut binary = 0usize;
    // Each unmodelled key, with the first frame that carried it.
    let mut unmodelled: BTreeMap<String, String> = BTreeMap::new();
    loop {
        let message = match timeout_at(deadline, socket.next()).await {
            Err(_) => break,
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(e))) => panic!("the socket failed: {e}"),
            Ok(None) => panic!("the socket ended without a close frame"),
        };
        let text = match message {
            Message::Text(text) => text,
            Message::Binary(_) => {
                binary += 1;
                continue;
            }
            Message::Close(frame) => {
                panic!("the server closed the socket: {}", describe_close(frame))
            }
            _ => continue,
        };
        let update = MatchUpdate::from_json(&text)
            .unwrap_or_else(|e| panic!("a live frame did not parse: {e}\n{}", text.as_str()));
        let original: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&update).unwrap(),
            original,
            "a live frame changed on a round trip:\n{}",
            text.as_str()
        );
        for key in update.extra.keys() {
            unmodelled
                .entry(key.clone())
                .or_insert_with(|| text.as_str().to_owned());
        }
        *leagues
            .entry(update.league_abbreviation.clone())
            .or_default() += 1;
    }
    assert_eq!(
        binary, 0,
        "the feed sent {binary} binary frames; this crate reads only text frames and would \
         skip them as mere liveness"
    );
    let checked: usize = leagues.values().sum();
    assert!(checked > 0, "no frames within {WIRE_WINDOW:?}; {QUIET}");
    assert!(
        unmodelled.is_empty(),
        "the feed sends keys MatchUpdate does not model; the first frame carrying each: \
         {unmodelled:#?}. Capture fixtures with scripts/capture_sports_fixtures.py, model the \
         keys in MatchUpdate, and record them in docs/specs/sports/OBSERVED.md"
    );
    println!("checked {checked} frames over {WIRE_WINDOW:?}, by league: {leagues:?}");
}
