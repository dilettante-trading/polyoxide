//! Live tests against wss://sports-api.polymarket.com/ws. Ignored by default:
//!
//! ```text
//! cargo test -p polyoxide-sports --test live_api -- --ignored --nocapture
//! ```
//!
//! The feed carries only matches that are live somewhere, so a test that times
//! out waiting for a frame fails as `environmental`. A bare stream that ends
//! shows no close code, so it fails as `transient`, which the nightly retries
//! to tell a restart from a defect. Every other failure carries the tag of its
//! `SportsError`: a server close by its close code, a raw socket's close frame
//! and transport error wrapped in the same error first. In the wire-agreement
//! test only a window with no data frame of any kind is environmental: binary
//! frames, which this crate does not read, fail there as a real fault.
//!
//! `nightly-schema.yml` excludes this host, because the published AsyncAPI
//! document does not match the wire. So
//! `live_frames_round_trip_and_carry_no_unmodelled_keys` is this host's drift
//! detector. It sees only what is live while it runs. The 06:00 UTC nightly
//! misses most North American leagues and weekend soccer, so the nightly also
//! runs at 18:30 UTC on Saturday and Sunday. That run still misses NBA and NHL
//! evening games; a capture in a busy window covers those.

use std::{collections::BTreeMap, time::Duration};

use futures_util::StreamExt;
use polyoxide_sports::{Event, MatchUpdate, SportsError, SportsWs, SportsWsBuilder, SPORTS_WS_URL};
use polyoxide_test_support::{environmental, fail, transient, ResultExt};
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
/// How long the wire-agreement test reads: long enough for most live games to
/// be re-sent at least once, though tennis repeats have come up to 153 s apart.
const WIRE_WINDOW: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const QUIET: &str = "if no matches are live anywhere this can legitimately time out, \
                     so re-run before concluding a defect";

type RawSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Fail a connect with its error's tag: a timeout or a 5xx is transient, a
/// refused upgrade real.
fn connect_failed<T>(error: SportsError) -> T {
    fail("could not connect to the sports feed", &error)
}

/// Open a raw socket, bounded like the crate's own connects. These tests call
/// `connect_async` directly, so they install the TLS provider the crate
/// would; see `ensure_crypto_provider` in src/client.rs.
async fn connect_raw() -> RawSocket {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let connected = match timeout(CONNECT_TIMEOUT, connect_async(SPORTS_WS_URL)).await {
        Ok(connected) => connected,
        Err(_) => connect_failed(SportsError::ConnectTimeout {
            after: CONNECT_TIMEOUT,
        }),
    };
    let (socket, _) = connected.unwrap_or_else(|e| {
        connect_failed(SportsError::Connect {
            source: Box::new(e),
        })
    });
    socket
}

/// A raw socket's close frame as the crate reports a server close, so the
/// failure takes its tag from the close code.
fn closed(frame: Option<CloseFrame>) -> SportsError {
    SportsError::Closed {
        code: frame.as_ref().map(|frame| u16::from(frame.code)),
        reason: frame
            .map(|frame| frame.reason.to_string())
            .unwrap_or_default(),
    }
}

/// A raw socket's transport error as the crate reports one.
fn transport(error: tokio_tungstenite::tungstenite::Error) -> SportsError {
    SportsError::Transport {
        source: Box::new(error),
    }
}

#[tokio::test]
#[ignore]
async fn live_bare_feed_yields_a_parsed_frame() {
    let mut feed = SportsWs::connect().await.unwrap_or_else(connect_failed);
    let update = timeout(RECV_WINDOW, feed.next())
        .await
        .unwrap_or_else(|_| environmental(&format!("no frame within {RECV_WINDOW:?}; {QUIET}")))
        .unwrap_or_else(|| transient("the server ended the connection instead of yielding a frame"))
        .or_fail("the frame parses");
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
            Ok(Some(Err(e @ SportsError::Decode { .. }))) => {
                fail(&format!("a frame did not parse after {frames} frames"), &e)
            }
            Ok(Some(Err(e))) => fail(&format!("the connection failed after {frames} frames"), &e),
            Ok(None) => transient(&format!(
                "the server ended the connection after {frames} frames, inside 40 s. An \
                 unanswered keep-alive would do this, and so would a server restart; the \
                 nightly retries it to tell which"
            )),
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
                fail("the server closed the socket", &closed(frame))
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => fail("the socket failed", &transport(e)),
            Ok(None) => transient("the server ended the connection without a close frame"),
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
            // Going stale is the failure under test, so it files whatever its
            // class; any other cause fails by its own.
            Ok(Some(Ok(Event::Disconnected {
                reason: reason @ SportsError::Stale { .. },
            }))) => {
                panic!("disconnected after {updates} updates: {reason}") // live-unwraps: the property under test
            }
            Ok(Some(Ok(Event::Disconnected { reason }))) => {
                fail(&format!("disconnected after {updates} updates"), &reason)
            }
            Ok(Some(Ok(other))) => panic!("unexpected event {other:?}"), // live-unwraps: an assertion on the event
            Ok(Some(Err(e @ SportsError::Decode { .. }))) => fail("a live frame did not parse", &e),
            Ok(Some(Err(e))) => fail("unexpected error", &e),
            // The supervised feed ends only after an error it cannot recover from.
            Ok(None) => panic!("the feed ended"), // live-unwraps: the supervised feed ended
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
            Ok(Some(Err(e))) => fail("the socket failed", &transport(e)),
            Ok(None) => transient("the server ended the connection without a close frame"),
        };
        let text = match message {
            Message::Text(text) => text,
            Message::Binary(_) => {
                binary += 1;
                continue;
            }
            Message::Close(frame) => fail("the server closed the socket", &closed(frame)),
            _ => continue,
        };
        let update = MatchUpdate::from_json(&text)
            .map_err(|source| SportsError::Decode {
                raw: text.to_string(),
                source,
            })
            .or_fail("a live frame did not parse");
        let original: Value = serde_json::from_str(&text).expect("a JSON frame"); // live-unwraps: the frame decoded above
        let round_trip = serde_json::to_value(&update).expect("an update serialises"); // live-unwraps: serialising test data
        assert_eq!(
            round_trip,
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
    if checked == 0 {
        environmental(&format!("no frames within {WIRE_WINDOW:?}; {QUIET}"));
    }
    assert!(
        unmodelled.is_empty(),
        "the feed sends keys MatchUpdate does not model. Capture fixtures with \
         scripts/capture_sports_fixtures.py, model the keys in MatchUpdate, and record them \
         in docs/specs/sports/OBSERVED.md. The first frame carrying each: {unmodelled:#?}"
    );
    println!("checked {checked} frames over {WIRE_WINDOW:?}, by league: {leagues:?}");
}
