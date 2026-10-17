//! Live WebSocket tests. Ignored by default — run with:
//!
//! ```text
//! cargo test -p polyoxide-polymarket --features clob-ws --test live_clob_ws -- --ignored --nocapture
//! ```
//!
//! Only a real connection catches a parser that silently discards every frame:
//! unit tests fed a fabricated frame pass regardless. The sports channel shipped
//! that way before it moved to the `sports` module, whose live suite now carries
//! its tests.

#![cfg(feature = "clob-ws")]

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use polyoxide_polymarket::clob::ws::{ClobWsError, WebSocket};
use polyoxide_polymarket::clob::{Account, ClobBuilder, Credentials};
#[cfg(feature = "keychain")]
use polyoxide_test_support::keychain;
use polyoxide_test_support::{fail, load_env, transient, ResultExt};
use polyoxide_venue::{class_for_close_code, Class};
use polyoxide_ws::{tungstenite::Message, WsError};

// ── User channel: is `markets` actually optional? ───────────────

/// How long the raw probe's connect may take, as the SDK's own.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Derive real L2 credentials from `POLYMARKET_PRIVATE_KEY` alone.
///
/// `/auth/derive-api-key` is signed with L1, which needs only the private key,
/// and returns the account's existing deterministic credential rather than
/// provisioning a new one. So this whole file needs one secret, not four.
/// Build the L1-signing account from `POLYMARKET_PRIVATE_KEY`, else the keychain.
///
/// L1 ignores the L2 credential entirely, so both sources reach the same
/// `derive_api_key` call below with placeholder credentials, there purely to
/// satisfy `Account`'s constructor. The derive path is what runs either way.
///
/// When neither source has the key, the loader fails the test as
/// `auth-gated`, which the nightly skips rather than filing an issue. An empty
/// value counts as unset, since the nightly passes an unset repository secret
/// as `""`, which would otherwise fail `Account::new` as a real failure.
fn l1_account() -> Account {
    let creds = load_env(&["POLYMARKET_PRIVATE_KEY"]);
    #[cfg(feature = "keychain")]
    let creds = creds.or_else(|_| {
        keychain(
            "polyoxide-clob",
            &[("POLYMARKET_PRIVATE_KEY", "private_key")],
        )
    });
    let creds = creds.unwrap_or_else(|missing| missing.or_auth_gated());
    Account::new(
        creds.get("POLYMARKET_PRIVATE_KEY"),
        Credentials {
            key: String::new(),
            secret: String::new(),
            passphrase: String::new(),
        },
    )
    .or_fail("build account from private key")
}

async fn derive_credentials() -> (String, String, String) {
    let account = l1_account();

    let clob = ClobBuilder::new()
        .with_account(account)
        .build()
        .or_fail("clob client");

    let resp = clob
        .auth()
        .or_fail("auth namespace")
        .derive_api_key(0)
        .send()
        .await
        .or_fail("derive_api_key should be accepted");

    (resp.api_key, resp.secret, resp.passphrase)
}

/// Open a raw user-channel socket, send `frame`, and report what the server did.
///
/// Deliberately raw rather than going through `UserSubscription`: the point is
/// to find out what the venue accepts *before* changing the SDK type, not to
/// confirm that a change we already made round-trips.
///
/// `Err` is the venue rejecting the frame. A failure that says nothing about
/// the frame fails the test from here with its own tag: a transport error by
/// its class, and a stream that ends, or closes with a code the socket table
/// treats as a dropped connection, as `transient`.
async fn probe_user_subscription(frame: String) -> Result<usize, String> {
    // A raw socket through the kit's connect, which installs the TLS
    // provider (DRIFT R9): this graph enables two rustls backends, so a
    // connect without it panics before it reaches the network. A connect
    // that times out is `Connection` whose kind is `ConnectTimeout`, classed
    // `Network`.
    let mut ws = polyoxide_ws::connect(
        "wss://ws-subscriptions-clob.polymarket.com/ws/user",
        CONNECT_TIMEOUT,
    )
    .await
    .map_err(ClobWsError::from)
    .or_fail("connect failed");

    ws.send(Message::Text(frame.into()))
        .await
        .map_err(|err| ClobWsError::from(WsError::from(err)))
        .or_fail("send failed");

    // A rejected subscription shows up as a close frame or an error payload
    // within the first few seconds. An accepted one simply stays quiet until
    // the account does something.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut frames = 0usize;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok(frames);
        }
        match tokio::time::timeout(remaining, ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let t = t.to_string();
                if t == "PONG" || t.trim().is_empty() || t == "{}" {
                    continue;
                }
                let lowered = t.to_lowercase();
                if lowered.contains("error") || lowered.contains("invalid") {
                    return Err(format!("server rejected the subscription: {t}"));
                }
                frames += 1;
            }
            Ok(Some(Ok(Message::Close(c)))) => {
                let code = c.as_ref().map(|frame| u16::from(frame.code));
                if matches!(class_for_close_code(code), Class::Network) {
                    transient(&format!("server closed the connection: {c:?}"));
                }
                return Err(format!("server closed the connection: {c:?}"));
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => fail("stream error", &ClobWsError::from(WsError::from(e))),
            Ok(None) => transient("the server ended the stream without a close frame"),
            Err(_) => return Ok(frames),
        }
    }
}

/// Settles whether `UserSubscription.markets` may be omitted.
///
/// asyncapi-user.json lists only `auth` and `type` as required and says
/// "If omitted, receives events for all markets" — but this session already
/// found two places where upstream's published contract does not match the
/// wire, so the SDK type is not changed on the strength of the document alone.
///
/// The control arm matters: if *both* frames are rejected the failure is the
/// credentials, not the omitted field.
#[tokio::test]
#[ignore]
async fn live_user_subscription_accepts_omitted_markets() {
    let (api_key, secret, passphrase) = derive_credentials().await;
    let auth = serde_json::json!({
        "apiKey": api_key,
        "secret": secret,
        "passphrase": passphrase,
    });

    // Control: the shape polyoxide sends today, which is known to work.
    let with_markets = serde_json::json!({
        "auth": auth,
        "type": "user",
        "markets": ["0xbd31dc8a20211944f6b70f31557f1001557b59905b7738480ca09bd4532f84af"],
    })
    .to_string();

    let control = probe_user_subscription(with_markets).await;
    assert!(
        control.is_ok(),
        "control subscription (with markets) failed, so this run proves nothing \
         about the omitted field — fix credentials first: {control:?}"
    );

    // The question under test.
    let without_markets = serde_json::json!({
        "auth": auth,
        "type": "user",
    })
    .to_string();

    match probe_user_subscription(without_markets).await {
        Ok(frames) => {
            println!(
                "server accepted a user subscription with `markets` omitted \
                 ({frames} event frames in the window)"
            );
        }
        Err(e) => {
            let finding = format!(
                "server did NOT accept an omitted `markets` field: {e}\n\
                 If this is reproducible, `markets` is genuinely required and the \
                 AsyncAPI mirror is wrong — leave the SDK type as Vec<String> and \
                 record the divergence."
            );
            panic!("{finding}"); // live-unwraps: the venue refusing the omitted field is the finding
        }
    }
}

/// The SDK's own `connect_user_all_markets` must be accepted by the venue.
///
/// The probe above tests a hand-built frame; this tests what the SDK actually
/// sends, so a serialization slip (`"markets": null`, or an empty array) cannot
/// pass unnoticed.
#[tokio::test]
#[ignore]
async fn live_connect_user_all_markets_is_accepted() {
    use polyoxide_polymarket::clob::ws::ApiCredentials;

    let (api_key, secret, passphrase) = derive_credentials().await;
    let mut ws =
        WebSocket::connect_user_all_markets(ApiCredentials::new(api_key, secret, passphrase))
            .await
            .or_fail("unfiltered user subscription should connect");

    // No events are expected on an idle account; what matters is that the
    // server does not reject or close the subscription.
    match tokio::time::timeout(Duration::from_secs(20), ws.next()).await {
        Err(_) => println!("connection held open for 20s with no market filter"),
        Ok(Some(Ok(msg))) => println!("received a user event: {msg:?}"),
        Ok(Some(Err(e))) => fail("server rejected the unfiltered subscription", &e),
        Ok(None) => transient("the server ended the connection on the unfiltered subscription"),
    }
}

// ── The supervised sockets ───────────────────────────────────────

use polyoxide_polymarket::clob::ws::{
    ApiCredentials, DisconnectReason, MarketEvent, MarketMessage, MarketWsBuilder, UserEvent,
    UserWsBuilder,
};
use polyoxide_polymarket::clob::Clob;
use polyoxide_test_support::environmental;

/// How long a subscribed token may take to deliver its first book. The server
/// answers a subscribe with the book at once, so 30 s is a stalled host.
const BOOK_WITHIN: Duration = Duration::from_secs(30);

/// How long the user socket must stay open without yielding an error item.
const STAY_CONNECTED: Duration = Duration::from_secs(15);

/// A token the CLOB lists as accepting orders, read from its own sampling
/// markets with the public client, so the token is one the market channel
/// serves. Fails as `environmental` when the CLOB lists none.
async fn a_listed_token() -> String {
    let listing = Clob::public()
        .markets()
        .sampling()
        .send()
        .await
        .or_fail("list the sampling markets");
    listing
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .find_map(|token| token.token_id.clone())
        .unwrap_or_else(|| {
            environmental(
                "the CLOB lists no sampling market accepting orders; market conditions \
                 rather than a defect",
            )
        })
}

/// The supervised market socket delivers the `book` of a token it subscribed
/// to, within [`BOOK_WITHIN`], with no error item on the way.
#[tokio::test]
#[ignore]
async fn live_supervised_market_socket_receives_a_book() {
    let token = a_listed_token().await;
    let mut ws = MarketWsBuilder::new()
        .connect(vec![token.clone()])
        .await
        .or_fail("connect the supervised market socket");

    let deadline = tokio::time::Instant::now() + BOOK_WITHIN;
    let mut seen_book = false;
    while !seen_book {
        let item = match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => transient("no book arrived within 30 s"),
            Ok(None) => transient("the supervised market socket ended before a book arrived"),
            Ok(Some(item)) => item,
        };
        if let MarketEvent::Message(MarketMessage::Book(book)) =
            item.or_fail("the supervised market socket yields an event")
        {
            assert_eq!(
                book.asset_id, token,
                "a book arrived for a token that was not subscribed"
            );
            seen_book = true;
        }
    }
    ws.close()
        .await
        .or_fail("close the supervised market socket");
}

/// The supervised user socket opens with the account's credentials and stays
/// connected for [`STAY_CONNECTED`]: the server refuses bad credentials about
/// a second after the subscribe with a 1008 close, which arrives as an error
/// item (`ClobWsError::Authentication`) and ends the stream, and any drop
/// inside the window fails the test.
#[tokio::test]
#[ignore]
async fn live_supervised_user_socket_stays_connected_with_the_accounts_credentials() {
    let (api_key, secret, passphrase) = derive_credentials().await;
    let mut ws = UserWsBuilder::new()
        .connect(None, ApiCredentials::new(api_key, secret, passphrase))
        .await
        .or_fail("connect the supervised user socket");

    let deadline = tokio::time::Instant::now() + STAY_CONNECTED;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => break,
            Ok(None) => transient("the supervised user socket ended inside the window"),
            Ok(Some(item)) => match item.or_fail("the supervised user socket yields an event") {
                UserEvent::Disconnected {
                    reason: DisconnectReason::Error(cause),
                } => fail("the supervised user socket lost its connection", &cause),
                UserEvent::Disconnected { reason } => transient(&format!(
                    "the supervised user socket lost its connection: {reason:?}"
                )),
                UserEvent::Reconnected => {
                    transient("the supervised user socket reconnected inside the window")
                }
                // An account event is not what this test asserts on.
                _ => {}
            },
        }
    }
    ws.close().await.or_fail("close the supervised user socket");
}
