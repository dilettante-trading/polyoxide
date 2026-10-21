//! Live tests against the demo host's socket, `#[ignore]`d. Run with:
//! ```sh
//! KALSHI_DEMO_API_KEY_ID=... KALSHI_DEMO_PRIVATE_KEY="$(cat demo.pem)" \
//!     cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored
//! ```
//!
//! The demo key signs the handshake; keys do not cross environments, so a production
//! key does not work. The market comes from the demo host's own listing over unsigned
//! REST, through `common::two_sided_market` or `common::two_sided_market_where`: open,
//! multivariate combinations excluded (`mve_filter=exclude`), with a book on both sides
//! confirmed on the orderbook route. With none, the tests are an environmental skip.
//!
//! `live_a_demo_market_delivers_an_uncrossed_snapshot` takes a market whose REST YES bid is
//! at least 0.60, since only then can its uncrossed check fail: were `use_yes_price` ignored,
//! the asks would be raw NO prices, which lie below 0.40 against such a bid, and the book
//! would read crossed. Against a YES bid under 0.50 a raw NO price can sit above it, and the
//! check would pass either way. With no such market the test is an environmental skip.
//!
//! `live_frames_carry_no_unmodelled_keys` is the socket's drift detector. It reads raw
//! frames through the kit, decodes each into its frame type, encodes it back, and fails on
//! a key the types do not model or a value that changes. A quiet feed passes: it checks
//! what arrives.
//!
//! Each failure prints the tag the nightly classifier reads, taken from its error's class:
//! a connect timeout, a 5xx or a reset is transient, a refused key is real, and a stream
//! that ends without saying why is transient.

#[allow(dead_code)]
mod common;

use std::{collections::BTreeSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        frames::{
            Delta, Rejected, Snapshot, Subscribed, TickerFrame, TradeFrame, Unsubscribed, Updated,
        },
        signed_request, Channel, Event, PredictionsMembership, PredictionsWsBuilder,
        PredictionsWsError, DEMO_WS,
    },
    Credentials, Predictions, DEMO,
};
use polyoxide_test_support::{agreement, environmental, load_env, transient, ResultExt};
use polyoxide_ws::{tungstenite::Message, Connection, Mode, DEFAULT_CONNECT_TIMEOUT};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use tokio::time::Instant;

/// How long the feed gets to say what it will.
const LISTEN: Duration = Duration::from_secs(30);

fn credentials() -> Credentials {
    let creds = load_env(&["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"])
        .unwrap_or_else(|missing| missing.or_auth_gated());
    Credentials::from_pem(
        creds.get("KALSHI_DEMO_API_KEY_ID"),
        creds.get("KALSHI_DEMO_PRIVATE_KEY"),
    )
    .or_fail("the demo key")
}

/// The demo host's unsigned REST client.
fn demo_client() -> Predictions {
    Predictions::builder()
        .base_url(DEMO)
        .build()
        .or_fail("the demo REST client")
}

/// An open market of the demo host, over unsigned REST. `two_sided_market` pages
/// `status=open&mve_filter=exclude` and confirms both sides of the book on the orderbook
/// route, and is an `environmental` skip when none qualifies.
async fn open_demo_ticker() -> Ticker {
    let market = common::two_sided_market(&demo_client()).await;
    Ticker::new(&market.ticker).or_fail("the demo market's ticker")
}

/// An open market of the demo host as `open_demo_ticker` finds one, whose REST YES bid is at
/// least 0.60. Against a bid that high every raw NO price lies below it (an uncrossed book
/// has NO bids under 1 minus the YES bid, so under 0.40), which is what lets the uncrossed
/// check fail. An `environmental` skip when none qualifies.
async fn high_bid_demo_ticker() -> Ticker {
    let floor = Decimal::new(60, 2);
    let market = common::two_sided_market_where(
        &demo_client(),
        ", with a YES bid of at least 0.60",
        |market| market.yes_bid_dollars >= floor,
    )
    .await;
    Ticker::new(&market.ticker).or_fail("the demo market's ticker")
}

/// The key paths the wire sent in `text` that its frame type does not write, or `None`
/// for a frame type this socket does not carry. Panics, untagged, on a value that changes.
fn unmodelled_keys(text: &str) -> Option<Vec<String>> {
    let kind = serde_json::from_str::<Value>(text)
        .ok()?
        .get("type")?
        .as_str()?
        .to_owned();
    let found = match kind.as_str() {
        "subscribed" => agreement::compare::<Subscribed>(&kind, text),
        "ok" => agreement::compare::<Updated>(&kind, text),
        "unsubscribed" => agreement::compare::<Unsubscribed>(&kind, text),
        "error" => agreement::compare::<Rejected>(&kind, text),
        "orderbook_snapshot" => agreement::compare::<Snapshot>(&kind, text),
        "orderbook_delta" => agreement::compare::<Delta>(&kind, text),
        _ => return None,
    };
    Some(
        found
            .unmodelled
            .into_iter()
            .map(|path| format!("{kind}: {path}"))
            .collect(),
    )
}

#[tokio::test]
#[ignore]
async fn live_a_demo_market_delivers_an_uncrossed_snapshot() {
    let credentials = credentials();
    let ticker = high_bid_demo_ticker().await;
    let mut feed = PredictionsWsBuilder::new()
        .url(DEMO_WS)
        .or_fail("the demo socket URL")
        .connect(credentials, [ticker])
        .await
        .or_fail("connect to the demo socket");
    let deadline = Instant::now() + LISTEN;
    let mut snapshot = None;
    while snapshot.is_none() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            break;
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before a snapshot"))
            .or_fail("the supervised demo feed");
        if let Event::Snapshot(book) = event {
            snapshot = Some(book);
        }
    }
    let book = snapshot.unwrap_or_else(|| transient("no orderbook_snapshot arrived within 30 s"));
    // `use_yes_price` puts both sides on one scale; a crossed book would mean it was not
    // honoured. The market was chosen for a YES bid of at least 0.60 so that this can fail:
    // raw NO prices would then lie below the bid (under 0.40), and the lowest ask would sit
    // under the best bid. A one-sided book has nothing to cross.
    if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
        assert!(
            bid.price < ask.price,
            "the book is crossed: the best bid {} is not below the best ask {}",
            bid.price,
            ask.price
        );
    }
    feed.close().await.or_fail("close the demo feed");
}

#[tokio::test]
#[ignore]
async fn live_frames_carry_no_unmodelled_keys() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let request = signed_request(&credentials, DEMO_WS).or_fail("sign the handshake");
    let socket = polyoxide_ws::connect(request, DEFAULT_CONNECT_TIMEOUT)
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("connect a raw socket through the kit");
    let mut connection = Connection::new(socket, Mode::Data);
    let subscribe = json!({"id": 1, "cmd": "subscribe", "params": {
        "channels": ["orderbook_delta"],
        "market_tickers": [ticker.as_str()],
        "use_yes_price": true,
    }});
    connection
        .send(Message::Text(subscribe.to_string().into()))
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("send the subscribe");
    let deadline = Instant::now() + LISTEN;
    let mut frames = 0usize;
    let mut unmodelled = BTreeSet::new();
    while let Ok(next) = tokio::time::timeout_at(deadline, connection.next()).await {
        let message = next
            .unwrap_or_else(|| transient("the server ended the connection"))
            .map_err(PredictionsWsError::from)
            .or_fail("read a frame");
        let Message::Text(text) = message else {
            continue;
        };
        frames += 1;
        if let Some(found) = unmodelled_keys(text.as_str()) {
            unmodelled.extend(found);
        }
    }
    assert!(frames > 0, "no frame arrived within 30 s");
    assert!(
        unmodelled.is_empty(),
        "the wire sent keys no frame type models: {unmodelled:?}"
    );
}

/// As `unmodelled_keys`, for every channel the socket carries: the two it does not read are
/// `ticker` and `trade`.
fn unmodelled_keys_on_any_channel(text: &str) -> Option<Vec<String>> {
    if let Some(found) = unmodelled_keys(text) {
        return Some(found);
    }
    let kind = serde_json::from_str::<Value>(text)
        .ok()?
        .get("type")?
        .as_str()?
        .to_owned();
    let found = match kind.as_str() {
        "ticker" => agreement::compare::<TickerFrame>(&kind, text),
        "trade" => agreement::compare::<TradeFrame>(&kind, text),
        _ => return None,
    };
    Some(
        found
            .unmodelled
            .into_iter()
            .map(|path| format!("{kind}: {path}"))
            .collect(),
    )
}

#[tokio::test]
#[ignore]
async fn live_three_channels_deliver_a_ticker_frame() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let mut feed = PredictionsWsBuilder::new()
        .url(DEMO_WS)
        .or_fail("the demo socket URL")
        .channels(Channel::ALL.iter().copied())
        .connect(credentials, [ticker.clone()])
        .await
        .or_fail("connect to the demo socket on every channel");
    let deadline = Instant::now() + LISTEN;
    let mut update = None;
    while update.is_none() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            break;
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before a ticker frame"))
            .or_fail("the supervised demo feed");
        if let Event::Ticker(found) = event {
            update = Some(found);
        }
    }
    let Some(update) = update else {
        environmental("no ticker frame within 30 s: the market was quiet");
    };
    assert_eq!(update.market_ticker, ticker.as_str());
    assert!(
        update.yes_bid <= update.yes_ask,
        "the ticker's bid {} is above its ask {}",
        update.yes_bid,
        update.yes_ask
    );
    feed.close().await.or_fail("close the demo feed");
}

#[tokio::test]
#[ignore]
async fn live_a_requested_snapshot_arrives() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let mut feed = PredictionsWsBuilder::new()
        .url(DEMO_WS)
        .or_fail("the demo socket URL")
        .connect(credentials, [ticker.clone()])
        .await
        .or_fail("connect to the demo socket");
    let membership = feed.membership();
    // The subscription's own snapshot comes first.
    let deadline = Instant::now() + LISTEN;
    loop {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            transient("no snapshot of the subscription within 30 s");
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before a snapshot"))
            .or_fail("the supervised demo feed");
        if let Event::Snapshot(book) = event {
            assert_eq!(book.market_ticker, ticker.as_str());
            break;
        }
    }
    membership
        .snapshot([ticker.clone()])
        .await
        .or_fail("ask for a snapshot");
    // Only the request can answer now: a snapshot that follows a reconnect or a re-snapshot
    // is the replay's or the gap's, and proves nothing about the request.
    let deadline = Instant::now() + LISTEN;
    let mut answered = false;
    while !answered {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            break;
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before the answer"))
            .or_fail("the supervised demo feed");
        match event {
            Event::Snapshot(book) => {
                assert_eq!(book.market_ticker, ticker.as_str());
                answered = true;
            }
            Event::Disconnected { .. } | Event::Reconnected | Event::Resyncing { .. } => {
                transient("the connection was interrupted before the requested snapshot came");
            }
            _ => {}
        }
    }
    // Kalshi answers every request, so a missing answer is a fault and not a quiet feed.
    assert!(answered, "no snapshot answered the request within 30 s");
    feed.close().await.or_fail("close the demo feed");
}

#[tokio::test]
#[ignore]
async fn live_frames_of_every_channel_carry_no_unmodelled_keys() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let request = signed_request(&credentials, DEMO_WS).or_fail("sign the handshake");
    let socket = polyoxide_ws::connect(request, DEFAULT_CONNECT_TIMEOUT)
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("connect a raw socket through the kit");
    let mut connection = Connection::new(socket, Mode::Data);
    let subscribe = json!({"id": 1, "cmd": "subscribe", "params": {
        "channels": ["orderbook_delta", "ticker", "trade"],
        "market_tickers": [ticker.as_str()],
        "use_yes_price": true,
    }});
    connection
        .send(Message::Text(subscribe.to_string().into()))
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("send the subscribe");
    let deadline = Instant::now() + LISTEN;
    let mut frames = 0usize;
    let mut unmodelled = BTreeSet::new();
    while let Ok(next) = tokio::time::timeout_at(deadline, connection.next()).await {
        let message = next
            .unwrap_or_else(|| transient("the server ended the connection"))
            .map_err(PredictionsWsError::from)
            .or_fail("read a frame");
        let Message::Text(text) = message else {
            continue;
        };
        frames += 1;
        if let Some(found) = unmodelled_keys_on_any_channel(text.as_str()) {
            unmodelled.extend(found);
        }
    }
    assert!(frames > 0, "no frame arrived within 30 s");
    assert!(
        unmodelled.is_empty(),
        "the wire sent keys no frame type models: {unmodelled:?}"
    );
}
