//! The supervised socket against the scripted server, with limits in hundreds of
//! milliseconds. Each test names the bug it exists to catch.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::StreamExt;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        DisconnectReason, Event, PredictionsMembership, PredictionsWsBuilder, PredictionsWsError,
        SupervisedPredictionsWs,
    },
    Credentials,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use polyoxide_ws::test_server::{next_event, next_item, WINDOW};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::{json, Value};

// Kalshi's answer on the shared scripted server, as the unit tests use it.
#[path = "../src/predictions/ws/scripted.rs"]
mod scripted;

use scripted::{delta, KalshiServer, Script};

const PATH: &str = "/trade-api/ws/v2";
const A: &str = "KXAAA-1";
const B: &str = "KXBBB-2";
const C: &str = "KXCCC-3";

fn t(text: &str) -> Ticker {
    Ticker::new(text).expect("a valid ticker")
}

fn vectors() -> Value {
    fixtures!("predictions/signing").json("vectors")
}

fn credentials() -> Credentials {
    let vectors = vectors();
    let body = vectors["ed25519"]["pkcs8_body"]
        .as_str()
        .expect("the vectors hold an Ed25519 key");
    // The fixture stores the key body without its armour lines (spec §6.4).
    let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n");
    Credentials::from_pem("test-key-id", &pem).expect("the vector key parses")
}

fn builder_for(server: &KalshiServer) -> PredictionsWsBuilder {
    PredictionsWsBuilder::new()
        .url(&format!("{}{PATH}", server.url))
        .expect("a loopback URL")
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}

fn kind(event: &Event) -> &'static str {
    match event {
        Event::Snapshot(_) => "snapshot",
        Event::Delta(_) => "delta",
        Event::Disconnected { .. } => "disconnected",
        Event::Reconnected => "reconnected",
        _ => "other",
    }
}

/// The kinds of the next `count` events.
async fn kinds(ws: &mut SupervisedPredictionsWs, count: usize) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for _ in 0..count {
        seen.push(kind(&next_event(ws).await));
    }
    seen
}

/// Asserts nothing arrives for `quiet`.
async fn assert_quiet(ws: &mut SupervisedPredictionsWs, quiet: Duration) {
    if let Ok(item) = tokio::time::timeout(quiet, ws.next()).await {
        panic!("expected silence, got {item:?}");
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> &'a str {
    headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .unwrap_or_else(|| panic!("no {name} header in {headers:?}"))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// Asserts the handshake's signature verifies over `timestamp + "GET" + PATH` with the
/// vector key's public key.
fn assert_signed(headers: &[(String, String)]) {
    let vectors = vectors();
    let public = hex(vectors["ed25519"]["public_key_hex"]
        .as_str()
        .expect("a public key"));
    let message = format!("{}GET{PATH}", header(headers, "kalshi-access-timestamp"));
    let signature = STANDARD
        .decode(header(headers, "kalshi-access-signature"))
        .expect("base64");
    UnparsedPublicKey::new(&ED25519, public)
        .verify(message.as_bytes(), &signature)
        .unwrap_or_else(|_| panic!("the signature over {message} does not verify"));
}

/// Asserts a request is a `subscribe` of exactly `expected`, in YES pricing.
fn assert_subscribe(frame: &Value, expected: &[&str]) {
    assert_eq!(frame["cmd"], "subscribe");
    assert_eq!(frame["params"]["channels"], json!(["orderbook_delta"]));
    assert_eq!(frame["params"]["use_yes_price"], true);
    let mut tickers: Vec<&str> = frame["params"]["market_tickers"]
        .as_array()
        .expect("a ticker list")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    tickers.sort_unstable();
    assert_eq!(tickers, expected);
}

#[tokio::test]
async fn every_attempt_signs_its_own_handshake() {
    // Bug: signing once, so that a reconnect replays a timestamp Kalshi refuses.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    let handshakes = server.handshake_headers();
    assert_eq!(handshakes.len(), 2);
    assert_ne!(
        header(&handshakes[0], "kalshi-access-timestamp"),
        header(&handshakes[1], "kalshi-access-timestamp")
    );
    for headers in &handshakes {
        assert_eq!(header(headers, "kalshi-access-key"), "test-key-id");
        assert_signed(headers);
    }
    assert_eq!(server.paths(), [PATH, PATH]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_reconnect_replays_every_market_in_one_subscribe() {
    // Bug: resubscribing to nothing, or to the first market only, after a reconnect.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(B), t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 6).await,
        [
            "snapshot",
            "snapshot",
            "disconnected",
            "reconnected",
            "snapshot",
            "snapshot"
        ]
    );
    let firsts = server.first_frames();
    assert_eq!(firsts.len(), 2);
    for first in &firsts {
        assert_subscribe(first, &[A, B]);
    }
    ws.close().await.expect("close");
}

#[tokio::test]
async fn changes_go_out_as_update_subscription_on_the_live_sid() {
    // Bug: a second `subscribe` per change, which opens a second subscription.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));

    membership.subscribe([t(B)]).await.expect("add");
    // The new market's snapshot follows the ok.
    match next_event(&mut ws).await {
        Event::Snapshot(book) => assert_eq!(book.market_ticker, B),
        other => panic!("expected the added market's snapshot, got {other:?}"),
    }
    membership.unsubscribe([t(A)]).await.expect("delete");
    membership
        .unsubscribe([t(B)])
        .await
        .expect("empty the membership");
    membership.subscribe([t(C)]).await.expect("add after empty");

    let requests = server.requests();
    assert_eq!(requests.len(), 5, "{requests:?}");
    assert_subscribe(&requests[0], &[A]);
    assert_eq!(requests[1]["cmd"], "update_subscription");
    assert_eq!(
        requests[1]["params"],
        json!({"sid": 1, "action": "add_markets", "market_tickers": [B]})
    );
    assert_eq!(
        requests[2]["params"],
        json!({"sid": 1, "action": "delete_markets", "market_tickers": [A]})
    );
    assert_eq!(requests[3]["cmd"], "unsubscribe");
    assert_eq!(requests[3]["params"], json!({"sids": [1]}));
    assert_subscribe(&requests[4], &[C]);
    // The kit's ids, counting from 1, one per command.
    let ids: Vec<u64> = requests.iter().filter_map(|r| r["id"].as_u64()).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn the_first_add_on_an_empty_membership_sends_a_subscribe() {
    // Bug: an `update_subscription` with no `sid`, or no frame at all.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), Vec::<Ticker>::new())
        .await
        .expect("connect");
    server
        .wait_for("a handshake", |s| s.handshake_headers().len() == 1)
        .await;
    assert!(server.requests().is_empty());
    let membership = ws.membership();
    membership.subscribe([t(A)]).await.expect("add");
    assert_subscribe(&server.requests()[0], &[A]);
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_refused_change_is_undone_and_not_replayed() {
    // Bug: a market Kalshi refused stays in the membership, so every replay carries it
    // and the subscription is refused for ever.
    let server = KalshiServer::start(vec![
        Script {
            refuse_ticker: Some(("KXBAD-1".to_owned(), 11)),
            cut_after: Some(Duration::from_millis(400)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    let err = membership
        .subscribe([t("KXBAD-1")])
        .await
        .expect_err("Kalshi refuses it");
    assert!(
        matches!(err, PredictionsWsError::Server { code: 11, .. }),
        "{err:?}"
    );
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["disconnected", "reconnected", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_change_made_during_an_outage_is_replayed() {
    // Bug: a change made while the connection is down is lost, or waits for ever.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    // A long backoff keeps the outage open while the change is made.
    let mut ws = builder_for(&server)
        .backoff(Duration::from_millis(200), Duration::from_millis(200))
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert_eq!(kinds(&mut ws, 2).await, ["snapshot", "disconnected"]);
    membership
        .subscribe([t(B)])
        .await
        .expect("recorded during the outage");
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    // Two subscribes and no update: the replay carried the change.
    assert_eq!(server.requests().len(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_change_whose_connection_dies_under_it_is_answered_ok_and_replayed() {
    // Bug: reporting the dead connection to the caller, who then retries a change the
    // replay is about to apply.
    let server = KalshiServer::start(vec![
        Script {
            ignore_updates: true,
            cut_after: Some(Duration::from_millis(200)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // Kalshi never answers the update; the server cuts the connection at 200 ms.
    membership.subscribe([t(B)]).await.expect("answered Ok");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["disconnected", "reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    ws.close().await.expect("close");
}

/// Asserts the next item is Kalshi's error `code`, and that the stream then ends.
async fn assert_ends_with_error(ws: &mut SupervisedPredictionsWs, code: u32) {
    match next_item(ws).await {
        Err(PredictionsWsError::Server { code: seen, .. }) => assert_eq!(seen, code),
        other => panic!("expected the server's error {code}, got {other:?}"),
    }
    let ended = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(ended.is_none());
}

#[tokio::test]
async fn a_replay_kalshi_refuses_ends_the_stream_when_the_change_was_made_in_an_outage() {
    // Bug: the change was answered Ok during the outage, so its caller never sees a
    // refusal, and the replay, refused on every connection, was retried at the backoff
    // ceiling for ever with only a WARN to show for it.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_ticker: Some((B.to_owned(), 11)),
            ..Default::default()
        },
    ])
    .await;
    // A long backoff keeps the outage open while the change is made.
    let mut ws = builder_for(&server)
        .backoff(Duration::from_millis(200), Duration::from_millis(200))
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert_eq!(kinds(&mut ws, 2).await, ["snapshot", "disconnected"]);
    membership
        .subscribe([t(B)])
        .await
        .expect("recorded during the outage");
    assert_ends_with_error(&mut ws, 11).await;
    // The replay was tried once and not again.
    assert_eq!(server.connection_count(), 2);
    assert_subscribe(&server.first_frames()[1], &[A, B]);
}

#[tokio::test]
async fn a_replay_kalshi_refuses_ends_the_stream_when_the_connection_died_under_the_change() {
    // Bug: as above, for a change whose connection died under it: it was answered Ok and
    // recorded, and the replay that carried it was refused for ever.
    let server = KalshiServer::start(vec![
        Script {
            ignore_updates: true,
            cut_after: Some(Duration::from_millis(200)),
            ..Default::default()
        },
        Script {
            refuse_ticker: Some((B.to_owned(), 11)),
            ..Default::default()
        },
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // Kalshi never answers the update; the server cuts the connection at 200 ms.
    membership.subscribe([t(B)]).await.expect("answered Ok");
    assert_eq!(kinds(&mut ws, 1).await, ["disconnected"]);
    assert_ends_with_error(&mut ws, 11).await;
    assert_eq!(server.connection_count(), 2);
    assert_subscribe(&server.first_frames()[1], &[A, B]);
}

#[tokio::test]
async fn a_replay_refused_with_a_code_that_reconnects_is_retried() {
    // Bug: ending the stream on every refused replay, a rate limit (27) included, which
    // the next connection's replay would have cleared.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_ticker: Some((A.to_owned(), 27)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    assert_eq!(server.connection_count(), 3);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_sequence_gap_reconnects_and_resnapshots() {
    // Bug: carrying on past a lost delta, so a book built on it is silently wrong.
    let server = KalshiServer::start(vec![
        Script {
            // The snapshot is seq 1; a delta at 3 skips 2.
            pushes: vec![delta(1, 3, A).to_string()],
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 2, 3)),
        other => panic!("expected the gap to end the connection, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_gap_in_an_answer_reconnects() {
    // Bug: checking only the pushes, so an `ok` that skipped a number goes unnoticed and
    // the delta lost behind it is never found.
    let server = KalshiServer::start(vec![
        Script {
            ok_skips_seq: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // The change itself succeeded; the gap behind it ends the connection.
    membership.subscribe([t(B)]).await.expect("the change");
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 2, 3)),
        other => panic!("expected the gap in the answer, got {other:?}"),
    }
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_delta_between_a_request_and_its_ok_is_not_a_gap() {
    // Bugs: counting the delta a request kept twice, and not counting it before the ok
    // behind it. Either reconnects on every change made while the book is moving.
    let server = KalshiServer::start(vec![Script {
        delta_before_ok: true,
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    membership.subscribe([t(B)]).await.expect("add");
    assert_eq!(kinds(&mut ws, 2).await, ["delta", "snapshot"]);
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn the_servers_pings_alone_keep_a_quiet_connection_live() {
    // Bug: counting only data as liveness, which drops a healthy connection in every
    // quiet hour. The client sends no pings here, so only the server's keep it live.
    let server = KalshiServer::start(vec![Script {
        ping_every: Some(Duration::from_millis(100)),
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .ping_interval(Duration::MAX)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // Three staleness windows of nothing but the server's pings.
    assert_quiet(&mut ws, Duration::from_millis(900)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_connection_with_no_traffic_at_all_goes_stale() {
    // The control for the test above: with no pings from either side the same connection
    // is replaced, so that test proves the pings are what kept it.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .ping_interval(Duration::MAX)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Stale { silent },
        } => assert!(silent >= Duration::from_millis(300), "{silent:?}"),
        other => panic!("expected a stall, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn error_code_9_ends_the_stream_with_its_error() {
    // Bug: reconnecting for ever, at the backoff ceiling, on credentials Kalshi refuses.
    let server = KalshiServer::start(vec![Script {
        error_after_subscribe: Some((9, "Authentication required".to_owned())),
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_item(&mut ws).await {
        Err(PredictionsWsError::Server { code, message }) => {
            assert_eq!(code, 9);
            assert_eq!(message, "Authentication required");
        }
        other => panic!("expected the server's error, got {other:?}"),
    }
    let ended = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(ended.is_none());
    assert_eq!(server.connection_count(), 1);
}

/// Kalshi reports `code` on the subscription, which its AsyncAPI calls terminal ("the user
/// must resubscribe"): the stream reports the outage and reconnects, and the replay
/// resubscribes.
async fn assert_error_code_reconnects(code: u32, message: &str) {
    let server = KalshiServer::start(vec![
        Script {
            error_after_subscribe: Some((code, message.to_owned())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Error(PredictionsWsError::Server { code: seen, .. }),
        } => assert_eq!(seen, code),
        other => panic!("expected code {code} to end the connection, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    assert_subscribe(&server.first_frames()[1], &[A]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn error_code_10_reconnects() {
    // Bug: yielding the channel error and carrying on with a subscription Kalshi has
    // ended, so the book goes stale with no outage marker.
    assert_error_code_reconnects(10, "Channel error").await;
}

#[tokio::test]
async fn error_code_25_reconnects() {
    // Bug: yielding the overflow and carrying on with a subscription Kalshi has ended.
    assert_error_code_reconnects(25, "Subscription buffer overflow").await;
}

#[tokio::test]
async fn error_code_17_reconnects() {
    // Bug: yielding the venue's own internal error and carrying on with a subscription it
    // may have lost, so the book goes stale with no outage marker.
    assert_error_code_reconnects(17, "Internal error").await;
}

#[tokio::test]
async fn error_code_18_reconnects() {
    // Bug: yielding the venue's own command timeout and carrying on, as above.
    assert_error_code_reconnects(18, "Command timeout").await;
}

#[tokio::test]
async fn an_error_frame_of_another_code_is_yielded_and_skipped() {
    // Bug: ending the stream, or reconnecting, on an error that is the caller's.
    let server = KalshiServer::start(vec![Script {
        error_after_subscribe: Some((11, "Invalid parameter".to_owned())),
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_item(&mut ws).await {
        Err(PredictionsWsError::Server { code, .. }) => assert_eq!(code, 11),
        other => panic!("expected the server's error, got {other:?}"),
    }
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn the_first_attempts_refusal_is_returned() {
    // Bug: retrying a key Kalshi refuses, so the caller never learns.
    let server = KalshiServer::start(vec![Script {
        refuse_upgrade: Some((401, Vec::new())),
        ..Default::default()
    }])
    .await;
    let err = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect_err("the upgrade is refused");
    assert_eq!(err.class(), Class::Unauthorized);
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn a_401_on_a_reconnect_ends_the_stream() {
    // Bug: reconnecting for ever on a refusal retrying cannot fix.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_upgrade: Some((401, Vec::new())),
            ..Default::default()
        },
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(kinds(&mut ws, 2).await, ["snapshot", "disconnected"]);
    match next_item(&mut ws).await {
        Err(err) => assert_eq!(err.class(), Class::Unauthorized),
        other => panic!("expected the refusal, got {other:?}"),
    }
    let ended = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(ended.is_none());
    assert_eq!(server.connection_count(), 2);
}

#[tokio::test]
async fn a_503_on_a_reconnect_is_retried() {
    // Bug: treating a host that is unwell as a refusal.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_upgrade: Some((503, Vec::new())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    assert_eq!(server.connection_count(), 3);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_retry_after_is_waited_out() {
    // Bug: reconnecting on the backoff's own schedule, which sends a request straight
    // back into the 429 and extends the ban Retry-After asked us to sit out.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_upgrade: Some((429, vec![("Retry-After".to_owned(), "1".to_owned())])),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    // The ceiling is 400 ms, so a one-second Retry-After is waited out up to it.
    let mut ws = builder_for(&server)
        .backoff(Duration::from_millis(10), Duration::from_millis(400))
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    let accepted = server.accepted_at();
    assert_eq!(accepted.len(), 3);
    let waited = accepted[2] - accepted[1];
    assert!(
        waited >= Duration::from_millis(350),
        "the retry came {waited:?} after the 429"
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn each_outage_yields_one_disconnected_and_one_reconnected() {
    // Bug: a marker missing, doubled, or one of the pair without the other.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 7).await,
        [
            "snapshot",
            "disconnected",
            "reconnected",
            "snapshot",
            "disconnected",
            "reconnected",
            "snapshot"
        ]
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_server_close_is_reported_with_its_code() {
    // Bug: a close with a code read as an error with none, or not reconnected at all.
    let server = KalshiServer::start(vec![
        Script {
            close_after: true,
            close_code: Some((1011, "internal error".to_owned())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Closed { code, reason },
        } => {
            assert_eq!(code, Some(1011));
            assert_eq!(reason, "internal error");
        }
        other => panic!("expected the close, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_frame_that_is_not_json_is_yielded_and_skipped() {
    // Bug: ending the stream, or reconnecting, on one frame Kalshi sent wrongly.
    let server = KalshiServer::start(vec![Script {
        pushes: vec!["not json".to_owned()],
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_item(&mut ws).await {
        Err(PredictionsWsError::Json { text, .. }) => assert_eq!(text, "not json"),
        other => panic!("expected a decode error, got {other:?}"),
    }
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_empty_membership_connects_sends_nothing_and_keeps_pinging() {
    // Bug: closing a connection that subscribes nothing, or letting it go stale.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), Vec::<Ticker>::new())
        .await
        .expect("connect");
    server
        .wait_for("two client pings", |s| s.ping_count() >= 2)
        .await;
    assert!(server.requests().is_empty());
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}
