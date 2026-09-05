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

/// Collect the symbols seen on one subscription within a time budget.
async fn symbols_seen(subscriptions: Vec<Subscription>, budget: Duration) -> HashSet<String> {
    let mut stream = Rtds::connect(subscriptions).await.expect("connect");
    let mut symbols = HashSet::new();
    let deadline = tokio::time::Instant::now() + budget;

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(update)))) => {
                symbols.insert(update.symbol().to_string());
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    symbols
}

/// A silent feed is ambiguous by itself: it happens both when upstream is
/// down and when our filter encoding regresses, and a stray space in the
/// `filters` JSON produces exactly the same symptom as an outage — the
/// snapshot still arrives, then updates never do. This test subscribes
/// filtered and unfiltered at once so "no frames" is only environmental when
/// the unfiltered control is silent too; a silent filter next to a live
/// control is a real failure.
#[tokio::test]
#[ignore]
async fn the_symbol_filter_actually_binds() {
    let topic = Topic::ChainlinkTwap(TwapWindow::Thirty);
    let budget = Duration::from_secs(25);

    let control = symbols_seen(vec![Subscription::for_topic(topic)], budget).await;
    let filtered = symbols_seen(Subscription::for_topic(topic).symbols(["btc/usd"]), budget).await;

    eprintln!("control symbols: {control:?}");
    eprintln!("filtered symbols: {filtered:?}");

    if control.is_empty() {
        panic!(
            "no frames on an unfiltered subscription in {budget:?}; upstream may \
             legitimately time out"
        );
    }

    assert!(
        !filtered.is_empty(),
        "the unfiltered control received {} symbol(s) but the filtered \
         subscription received none — the filter encoding is broken, which is \
         exactly what a stray space in `filters` looks like",
        control.len()
    );
    assert_eq!(
        filtered,
        HashSet::from(["btc/usd".to_string()]),
        "the filter must bind to exactly one symbol; control saw {control:?}"
    );
    assert!(
        control.len() > 1,
        "an unfiltered subscription should see several symbols, saw {control:?}"
    );
}

/// A TWAP update's `value` must decode to a plausible BTC price. A scale
/// error is invisible to type checking — it shows up only as a magnitude
/// that is wrong by a power of ten (or eighteen), which is exactly what an
/// E18 misdecode looks like.
#[tokio::test]
#[ignore]
async fn twap_updates_decode_to_a_plausible_price() {
    let budget = Duration::from_secs(20);
    let mut stream = Rtds::connect(
        Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Sixty)).symbols(["btc/usd"]),
    )
    .await
    .expect("connect");

    let deadline = tokio::time::Instant::now() + budget;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(update)))) => {
                // A scale error shows up here as a wildly wrong magnitude:
                // an E18 misdecode reads ~8e-14, the reverse reads ~8e22.
                let value = update.value();
                eprintln!("decoded 60s BTC TWAP: {value}");
                assert!(
                    value > rust_decimal::Decimal::from(100u32)
                        && value < rust_decimal::Decimal::from(10_000_000u32),
                    "BTC TWAP decoded to {value}, which is not a plausible price — \
                     check the E18 scale"
                );
                assert_eq!(update.window(), Some(TwapWindow::Sixty));
                return;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    panic!("no TWAP updates in {budget:?}; upstream may legitimately time out");
}
