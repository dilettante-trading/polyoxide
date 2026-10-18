//! Live tests against the demo host's socket, `#[ignore]`d. Run with:
//! ```sh
//! KALSHI_DEMO_API_KEY_ID=... KALSHI_DEMO_PRIVATE_KEY="$(cat demo.pem)" \
//!     cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored
//! ```
//!
//! The demo key signs the handshake; keys do not cross environments, so a production
//! key does not work. The market comes from the demo host's own listing over unsigned
//! REST, through `common::two_sided_market`: open, multivariate combinations excluded
//! (`mve_filter=exclude`), with a book on both sides confirmed on the orderbook route. With
//! none, the tests are an environmental skip.
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
        frames::{Delta, Rejected, Snapshot, Subscribed, Unsubscribed, Updated},
        signed_request, Event, PredictionsWsBuilder, PredictionsWsError, DEMO_WS,
    },
    Credentials, Predictions, DEMO,
};
use polyoxide_test_support::{agreement, load_env, transient, ResultExt};
use polyoxide_ws::{tungstenite::Message, Connection, Mode, DEFAULT_CONNECT_TIMEOUT};
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

/// An open market of the demo host, over unsigned REST. `two_sided_market` pages
/// `status=open&mve_filter=exclude` and confirms both sides of the book on the orderbook
/// route, and is an `environmental` skip when none qualifies.
async fn open_demo_ticker() -> Ticker {
    let client = Predictions::builder()
        .base_url(DEMO)
        .build()
        .or_fail("the demo REST client");
    let market = common::two_sided_market(&client).await;
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
    let ticker = open_demo_ticker().await;
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
    // honoured. A one-sided book has nothing to cross.
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
