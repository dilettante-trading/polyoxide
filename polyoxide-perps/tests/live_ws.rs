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
    handle
        .subscribe([Channel::Trades(iid)])
        .await
        .expect("subscribe trades");
    handle
        .unsubscribe([Channel::Tickers(None)])
        .await
        .expect("unsubscribe tickers");
    // Ticker frames that were already queued when the server acknowledged
    // the unsubscribe are still yielded: the supervised tier buffers up to
    // 1024 events, and the bare tier parks every push it reads while waiting
    // for the acknowledgement. Nothing consumed during the two round trips
    // above, so the queue holds about a thousand fan-out frames (1055 on the
    // first live run), and a count-after-a-sleep cannot tell a full queue
    // from a server that ignored the request. Timing can: a local queue
    // drains in milliseconds, while `tickers::all` keeps arriving at
    // hundreds of frames a second if the subscription is still live.
    let acknowledged = tokio::time::Instant::now();
    let deadline = acknowledged + Duration::from_secs(5);
    let mut drained = 0;
    let mut last_ticker = None;
    while let Ok(Some(Ok(event))) = tokio::time::timeout_at(deadline, ws.next()).await {
        if matches!(event, Event::Update(ref u) if matches!(u.channel, Channel::Tickers(_))) {
            drained += 1;
            last_ticker = Some(tokio::time::Instant::now());
        }
    }
    let quiet_after = last_ticker.map(|at| at.duration_since(acknowledged));
    eprintln!(
        "drained {drained} queued tickers; last one {quiet_after:?} after the acknowledgement"
    );
    assert!(
        quiet_after.is_none_or(|d| d < Duration::from_secs(2)),
        "tickers kept arriving after unsubscribe: {drained} in the window, last at {quiet_after:?}"
    );
    ws.close().await.expect("close");
}
