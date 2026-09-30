//! Live WebSocket tests against the Polymarket Perps host. `#[ignore]`d;
//! run with:
//! ```sh
//! cargo test -p polyoxide-perps --features ws --test live_ws -- --ignored
//! ```

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_perps::{
    types::{BookDepth, InstrumentId},
    ws::{Channel, Event, Frame, PerpsWs, PerpsWsBuilder, StreamDepth},
    Perps,
};

/// An instrument with a two-sided book right now, chosen on the REST route
/// the socket mirrors. Panics with the environmental phrasing if none.
async fn a_quoting_instrument() -> InstrumentId {
    let perps = Perps::new().unwrap();
    for instrument in perps
        .exchange()
        .instruments()
        .send()
        .await
        .expect("instruments")
    {
        if let Ok(book) = perps
            .market()
            .book(instrument.instrument_id)
            .depth(BookDepth::Ten)
            .send()
            .await
        {
            if !book.bids.is_empty() && !book.asks.is_empty() {
                return instrument.instrument_id;
            }
        }
    }
    panic!("no suitable market: no instrument has a two-sided book right now");
}

#[tokio::test]
#[ignore]
async fn live_bare_stream_delivers_book_and_bbo_frames_and_answers_ping() {
    let iid = a_quoting_instrument().await;
    let mut ws = PerpsWs::connect([Channel::Book(iid, StreamDepth::Twenty), Channel::Bbo(iid)])
        .await
        .expect("connect");
    ws.ping().await.expect("pong");
    let mut seen_book = false;
    let mut seen_bbo = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !(seen_book && seen_bbo) {
        let frame = tokio::time::timeout_at(deadline, ws.next()).await;
        let Ok(Some(frame)) = frame else { break };
        if let Frame::Update(u) = frame.expect("frame decodes") {
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
    assert!(
        seen_book && seen_bbo,
        "book and bbo frames may legitimately time out on a quiet instrument"
    );
    ws.close().await.expect("close");
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
        .expect("connect");
    let handle = ws.membership();
    let first = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("a ticker within 10 s")
        .unwrap()
        .unwrap();
    match first {
        Event::Update(u) => assert!(
            matches!(u.channel, Channel::Tickers(Some(_))),
            "fan-out label, got {}",
            u.channel
        ),
        other => panic!("expected a ticker update, got {other:?}"),
    }
    // `tickers::all` fills the supervised tier's 1024-event buffer in about
    // a second, and a task parked in a full buffer cannot take a membership
    // command. So the handle calls run on their own task while this one
    // keeps draining, and the acknowledgement time comes back on a oneshot.
    let (acked_tx, mut acked_rx) = tokio::sync::oneshot::channel();
    let changes = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .subscribe([Channel::Trades(iid)])
                .await
                .expect("subscribe trades");
            handle
                .unsubscribe([Channel::Tickers(None)])
                .await
                .expect("unsubscribe tickers");
            let _ = acked_tx.send(tokio::time::Instant::now());
        }
    });
    let mut before_ack = 0;
    let mut acknowledged = None;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                event = ws.next() => {
                    let event = event
                        .expect("stream ended during the membership change")
                        .expect("event decodes");
                    if is_ticker(&event) {
                        before_ack += 1;
                    }
                }
                at = &mut acked_rx => {
                    // An `Err` means the task panicked; joining it below
                    // surfaces which call failed.
                    acknowledged = at.ok();
                    break;
                }
            }
        }
    })
    .await
    .expect("membership changes acknowledged within 20 s");
    changes.await.expect("membership task");
    let acknowledged = acknowledged.expect("acknowledgement time");

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
            Ok(None) => panic!("stream ended during the drain window"),
            Ok(Some(Err(err))) => panic!("stream error during the drain window: {err}"),
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
    ws.close().await.expect("close");
}
