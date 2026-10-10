//! Live WebSocket tests against the Polymarket Perps host. `#[ignore]`d;
//! run with:
//! ```sh
//! cargo test -p polyoxide-perps --features ws --test live_ws -- --ignored
//! ```

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_perps::{
    types::{BookDepth, InstrumentId},
    ws::{Channel, Event, Frame, PerpsWs, PerpsWsBuilder, PerpsWsError, StreamDepth},
    Perps,
};
use polyoxide_test_support::{environmental, fail, transient, ResultExt};

/// An instrument with a two-sided book right now, chosen on the REST route
/// the socket mirrors. Fails as environmental if none, and with the last
/// probe's error if every probe errored, since that is an outage rather than
/// a quiet market.
async fn a_quoting_instrument() -> InstrumentId {
    let perps = Perps::new().or_fail("perps client");
    let mut answered = false;
    let mut last_error = None;
    for instrument in perps
        .exchange()
        .instruments()
        .send()
        .await
        .or_fail("instruments")
    {
        match perps
            .market()
            .book(instrument.instrument_id)
            .depth(BookDepth::Ten)
            .send()
            .await
        {
            Ok(book) => {
                if !book.bids.is_empty() && !book.asks.is_empty() {
                    return instrument.instrument_id;
                }
                answered = true;
            }
            Err(err) => last_error = Some(err),
        }
    }
    if let (false, Some(err)) = (answered, &last_error) {
        fail("every book probe failed", err);
    }
    environmental("no suitable market: no instrument has a two-sided book right now");
}

#[tokio::test]
#[ignore]
async fn live_bare_stream_delivers_book_and_bbo_frames_and_answers_ping() {
    let iid = a_quoting_instrument().await;
    let mut ws = PerpsWs::connect([Channel::Book(iid, StreamDepth::Twenty), Channel::Bbo(iid)])
        .await
        .or_fail("connect");
    ws.ping().await.or_fail("pong");
    let mut seen_book = false;
    let mut seen_bbo = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !(seen_book && seen_bbo) {
        let frame = match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => break,
            // The bare stream hides the close code, so a retry tells a
            // restart from a defect.
            Ok(None) => transient("the server ended the connection before book and bbo arrived"),
            Ok(Some(frame)) => frame,
        };
        if let Frame::Update(u) = frame.or_fail("frame decodes") {
            match u.channel {
                Channel::Book(..) => seen_book = true,
                Channel::Bbo(_) => seen_bbo = true,
                _ => {}
            }
            assert!(u.ets.is_some(), "every push frame carries ets");
        }
    }
    // A quoting instrument pushes book and bbo several times a second; a
    // quiet spell long enough to miss both in 15 s would legitimately time
    // out, so classify it as environmental rather than a defect.
    if !(seen_book && seen_bbo) {
        environmental("book and bbo frames may legitimately time out on a quiet instrument");
    }
    ws.close().await.or_fail("close");
}

fn is_ticker(event: &Event) -> bool {
    matches!(event, Event::Update(u) if matches!(u.channel, Channel::Tickers(_)))
}

#[tokio::test]
#[ignore]
async fn live_all_tickers_fans_out_per_instrument_and_membership_changes_apply() {
    let iid = a_quoting_instrument().await;
    let mut ws = PerpsWsBuilder::new()
        .connect([Channel::Tickers(None)])
        .await
        .or_fail("connect");
    let handle = ws.membership();
    // `tickers::all` never goes quiet for 10 s, so silence here is a fault, and
    // the supervised stream ends only after a fatal error.
    let first = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("a ticker within 10 s") // live-unwraps: silence is the fault under test
        .expect("the supervised stream ended") // live-unwraps: it ends only after a fatal error
        .or_fail("first event decodes");
    match first {
        Event::Update(u) => assert!(
            matches!(u.channel, Channel::Tickers(Some(_))),
            "fan-out label, got {}",
            u.channel
        ),
        other => panic!("expected a ticker update, got {other:?}"), // live-unwraps: an assertion on the event
    }
    // `tickers::all` fills the supervised tier's 1024-event buffer in about
    // a second, and a task parked in a full buffer cannot take a membership
    // command. So the handle calls run on their own task while this one
    // keeps draining, and the acknowledgement time comes back on a oneshot.
    // The task returns a failed call's error rather than panicking, so the
    // test fails with it, tagged by its class.
    let (acked_tx, mut acked_rx) = tokio::sync::oneshot::channel();
    let changes = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .subscribe([Channel::Trades(iid)])
                .await
                .map_err(|err| ("subscribe trades", err))?;
            handle
                .unsubscribe([Channel::Tickers(None)])
                .await
                .map_err(|err| ("unsubscribe tickers", err))?;
            let _ = acked_tx.send(tokio::time::Instant::now());
            Ok::<(), (&str, PerpsWsError)>(())
        }
    });
    let mut before_ack = 0;
    let mut acknowledged = None;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                event = ws.next() => {
                    let event = event
                        .expect("stream ended during the membership change") // live-unwraps: the supervised stream ends only after a fatal error
                        .or_fail("event decodes");
                    if is_ticker(&event) {
                        before_ack += 1;
                    }
                }
                at = &mut acked_rx => {
                    // An `Err` means a membership call failed; joining the
                    // task below fails the test with its error.
                    acknowledged = at.ok();
                    break;
                }
            }
        }
    })
    .await
    .expect("membership changes acknowledged within 20 s"); // live-unwraps: an unacknowledged change is the fault under test
    let changed = changes.await.expect("membership task"); // live-unwraps: a `JoinError` in test code
    if let Err((ctx, err)) = changed {
        fail(ctx, &err);
    }
    let acknowledged = acknowledged.expect("acknowledgement time"); // live-unwraps: set whenever the task succeeded

    // Frames received before the acknowledgement say nothing about whether
    // the server honoured it: the first live run counted 1,055 of them with
    // nothing draining, all received before the acknowledgement. Timing
    // does: if the subscription were still live, `tickers::all` would keep
    // arriving at hundreds of frames a second for the whole window.
    let deadline = acknowledged + Duration::from_secs(5);
    let mut after_ack = 0;
    let mut last_ticker = None;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => break,
            // The supervised stream ends only after a fatal error.
            Ok(None) => panic!("stream ended during the drain window"), // live-unwraps: a fault on the supervised tier
            Ok(Some(Err(err))) => fail("stream error during the drain window", &err),
            Ok(Some(Ok(event))) => {
                if is_ticker(&event) {
                    after_ack += 1;
                    last_ticker = Some(tokio::time::Instant::now());
                }
            }
        }
    }
    let quiet_after = last_ticker.map(|at| at.duration_since(acknowledged));
    eprintln!(
        "{before_ack} tickers before the acknowledgement, {after_ack} after; last one {quiet_after:?} after it"
    );
    assert!(
        quiet_after.is_none_or(|d| d < Duration::from_secs(2)),
        "tickers kept arriving after unsubscribe: {after_ack} in the window, last at {quiet_after:?}"
    );
    ws.close().await.or_fail("close");
}
