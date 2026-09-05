//! Tests against the real RTDS host. Ignored by default; run with
//! `cargo test -p polyoxide-rtds --test live_api -- --ignored`.

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_rtds::{PriceEvent, Rtds, Subscription, Topic, TwapWindow};

/// Answers the design's open question: does RTDS accept a second subscribe
/// frame on an open connection?
///
/// If this passes, `Rtds::subscribe_more` is worth adding. If it fails,
/// changing subscriptions means reconnecting, and no such method should exist.
#[tokio::test]
#[ignore]
async fn reports_whether_a_second_subscribe_frame_is_accepted() {
    let mut stream = Rtds::connect(
        Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"]),
    )
    .await
    .expect("connect");

    // Drain the first topic's traffic briefly to confirm the feed is alive.
    let mut saw_first = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(_)))) => {
                saw_first = true;
                break;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => panic!("stream ended early"),
            Err(_) => continue,
        }
    }

    assert!(
        saw_first,
        "no updates on an unfiltered-cadence topic in 20s; the feed may \
         legitimately time out if upstream is down"
    );

    // Now send a second subscribe frame on the same open connection, widening
    // to a different topic, and see whether the venue honours it.
    match stream
        .subscribe_more(Subscription::for_topic(Topic::ChainlinkSpot).symbols(["btc/usd"]))
        .await
    {
        Ok(()) => {}
        Err(err) => panic!("subscribe_more was rejected outright: {err}"),
    }

    // Record which topics produce update frames over the next 30 seconds.
    let mut topics_seen: HashSet<Topic> = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut server_rejection: Option<String> = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(update)))) => {
                topics_seen.insert(update.topic());
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(polyoxide_rtds::RtdsError::Server {
                status_code,
                message,
            }))) => {
                // A third possibility besides "accepted" and "silently
                // ignored": the venue can reject the second subscribe frame
                // outright with its error envelope.
                server_rejection = Some(format!("status {status_code}: {message}"));
                break;
            }
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => break,
            Err(_) => continue,
        }
        if topics_seen.contains(&Topic::ChainlinkTwap(TwapWindow::Thirty))
            && topics_seen.contains(&Topic::ChainlinkSpot)
        {
            break;
        }
    }

    eprintln!("topics seen after subscribe_more: {topics_seen:?}");
    if let Some(rejection) = &server_rejection {
        eprintln!("second subscribe frame was rejected by the venue: {rejection}");
    }

    assert!(
        server_rejection.is_none(),
        "RTDS rejected the second subscribe frame outright: {}",
        server_rejection.unwrap_or_default()
    );
    assert!(
        topics_seen.contains(&Topic::ChainlinkTwap(TwapWindow::Thirty)),
        "the original subscription stopped producing frames: {topics_seen:?}"
    );
    assert!(
        topics_seen.contains(&Topic::ChainlinkSpot),
        "no frames arrived from the topic added via subscribe_more in 30s: {topics_seen:?}; \
         RTDS may not honour a second subscribe frame on an open connection"
    );
}
