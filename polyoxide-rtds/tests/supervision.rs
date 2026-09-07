//! Supervision behaviour, exercised against a local scripted server.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use polyoxide_rtds::test_server::{Script, ScriptedServer};
use polyoxide_rtds::{PriceEvent, RtdsBuilder, Subscription, Topic, TwapWindow};

// The same golden vector the unit tests use, not a second copy. `fixtures`
// is behind the `test-fixtures` feature precisely so integration tests can
// reach it — a pasted duplicate would silently diverge on the next capture.
use polyoxide_rtds::fixtures::TWAP_THIRTY_UPDATE as TWAP_UPDATE;

fn subs() -> Vec<Subscription> {
    Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"])
}

#[tokio::test]
async fn reconnects_and_resubscribes_after_a_drop() {
    let server = ScriptedServer::start(vec![
        Script::SendThenClose(vec![TWAP_UPDATE.into()]),
        // The rejection is Fatal, so `run` returns on its own rather than
        // being cut off by a timeout — which also means the assertions below
        // observe a supervisor that actually finished.
        Script::SendThenIdle(vec![
            TWAP_UPDATE.into(),
            polyoxide_rtds::fixtures::REJECTED_SUBSCRIPTION.into(),
        ]),
    ])
    .await;

    let seen = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&seen);

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_secs(60))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        supervised.run(move |event| {
            let counter = Arc::clone(&counter);
            async move {
                if matches!(event, PriceEvent::Update(_)) {
                    *counter.lock().unwrap() += 1;
                }
                Ok(())
            }
        }),
    )
    .await
    .expect("run must return on a rejection rather than hang");

    assert!(
        outcome.is_err(),
        "the second connection's rejection must surface"
    );

    assert!(
        server.connection_count() >= 2,
        "expected a reconnect, saw {} connection(s)",
        server.connection_count()
    );
    assert!(
        *seen.lock().unwrap() >= 2,
        "expected updates from both connections"
    );

    // The resubscribe must send the same frame, or the new connection is
    // subscribed to nothing and goes quiet without erroring.
    let frames = server.received_subscriptions();
    assert!(frames.len() >= 2, "expected 2 subscription frames");
    assert_eq!(
        frames[0], frames[1],
        "resubscribe must replay the original subscription exactly"
    );
    assert!(
        frames[0].contains(r#"{\"symbol\":\"btc/usd\"}"#),
        "{}",
        frames[0]
    );
}

#[tokio::test]
async fn a_silent_connection_is_treated_as_dead() {
    let server = ScriptedServer::start(vec![
        // Nothing sent, never closed: the only thing that can end this
        // connection is the staleness timer.
        Script::SendThenIdle(Vec::new()),
        // Once the watchdog has forced a reconnect, end the run naturally
        // rather than leaning on an outer timeout.
        Script::SendThenIdle(vec![polyoxide_rtds::fixtures::REJECTED_SUBSCRIPTION.into()]),
    ])
    .await;

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_millis(200))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        supervised.run(|_event| async move { Ok(()) }),
    )
    .await
    .expect("the staleness watchdog must fire and the run must then end");

    assert!(
        outcome.is_err(),
        "the rejection on the second connection is fatal"
    );
    assert!(
        server.connection_count() >= 2,
        "a stalled connection must be reconnected, saw {}",
        server.connection_count()
    );
}

#[tokio::test]
async fn a_rejected_subscription_stops_instead_of_looping() {
    // One unrecognised topic zeroes an entire batch. Retrying replays the same
    // rejection forever, so the run loop must give up and return the error.
    // The captured envelope, not a hand-rolled lookalike — the other two
    // tests already use it, and a second copy would drift on the next capture.
    let server = ScriptedServer::start(vec![Script::SendThenIdle(vec![
        polyoxide_rtds::fixtures::REJECTED_SUBSCRIPTION.into(),
    ])])
    .await;

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_secs(60))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        supervised.run(|_event| async move { Ok(()) }),
    )
    .await
    .expect("run must return rather than retry forever");

    assert!(outcome.is_err(), "a rejected subscription must surface");
    assert_eq!(
        server.connection_count(),
        1,
        "must not reconnect into a rejected subscription"
    );
}

/// A frame this client cannot read must not end a 24/7 feed.
///
/// `error.rs` proves a bad frame classifies as [`Recovery::SkipFrame`]; this
/// proves the supervisor acts on that. Without it the classifier can be
/// correct while `pump` still drops the connection, and the two spellings are
/// indistinguishable from every other test in the suite.
#[tokio::test]
async fn a_frame_the_client_cannot_read_is_skipped_without_dropping_the_connection() {
    // Hand-built rather than captured: the venue has never sent either of
    // these. The first fails to parse as a frame at all (`RtdsError::Json`);
    // the second parses but carries a plain decimal on an E18 topic
    // (`RtdsError::Precision`). They are the two distinct routes to SkipFrame,
    // and a `pump` that handled only one would still look correct.
    const UNPARSEABLE: &str = "{not json";
    const UNDECODABLE: &str = r#"{"payload":{"full_accuracy_value":"79697.47",
        "symbol":"btc/usd","timestamp":1788600388000,"value":79697.47,"window_s":30},
        "timestamp":1788600389537,"topic":"crypto_prices_twap_thirty","type":"update"}"#;

    let server = ScriptedServer::start(vec![Script::SendThenIdle(vec![
        TWAP_UPDATE.into(),
        UNPARSEABLE.into(),
        UNDECODABLE.into(),
        TWAP_UPDATE.into(),
        // Ends the run deliberately, so the assertions below observe a
        // supervisor that finished rather than one cut off by a timeout.
        polyoxide_rtds::fixtures::REJECTED_SUBSCRIPTION.into(),
    ])])
    .await;

    let seen = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&seen);

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_secs(60))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        supervised.run(move |event| {
            let counter = Arc::clone(&counter);
            async move {
                if matches!(event, PriceEvent::Update(_)) {
                    *counter.lock().unwrap() += 1;
                }
                Ok(())
            }
        }),
    )
    .await
    .expect("run must return on the rejection rather than hang");

    // The run ends on the *rejection*, not on either bad frame. If a bad frame
    // ended it, this would be Json or Precision instead.
    match outcome {
        Err(polyoxide_rtds::RtdsError::Server { .. }) => {}
        other => {
            panic!("the run must survive both bad frames and end on the rejection, got {other:?}")
        }
    }

    assert_eq!(
        *seen.lock().unwrap(),
        2,
        "both updates must arrive — the ones either side of the unreadable frames"
    );
    assert_eq!(
        server.connection_count(),
        1,
        "a bad frame is not a bad connection; there must be no reconnect"
    );
}
