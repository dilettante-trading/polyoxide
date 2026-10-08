//! Live WebSocket tests. Ignored by default — run with:
//!
//! ```text
//! cargo test -p polyoxide-clob --features ws --test live_ws -- --ignored --nocapture
//! ```
//!
//! Only a real connection catches a parser that silently discards every frame:
//! unit tests fed a fabricated frame pass regardless. The sports channel shipped
//! that way before it moved to `polyoxide-sports`, whose live suite now carries
//! its tests.

#![cfg(feature = "ws")]

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use polyoxide_clob::ws::WebSocket;
use polyoxide_clob::{Account, ClobBuilder, Credentials};

// ── User channel: is `markets` actually optional? ───────────────

/// Derive real L2 credentials from `POLYMARKET_PRIVATE_KEY` alone.
///
/// `/auth/derive-api-key` is signed with L1, which needs only the private key,
/// and returns the account's existing deterministic credential rather than
/// provisioning a new one. So this whole file needs one secret, not four.
/// Build the L1-signing account from `POLYMARKET_PRIVATE_KEY`, else the keychain.
///
/// L1 ignores the L2 credential entirely, so both sources reach the same
/// `derive_api_key` call below: the env leg supplies placeholder credentials
/// purely to satisfy `Account`'s constructor, and the keychain leg happens to
/// carry a real triple that is then ignored. The derive path is what runs
/// either way.
///
/// The panic keeps the phrase `POLYMARKET_PRIVATE_KEY required` verbatim:
/// `AUTH_GATED_RE` in `.github/scripts/classify_failures.py` matches on it to
/// skip these in the nightly rather than filing an issue. An empty value counts
/// as unset, since the nightly passes an unset repository secret as `""`, which
/// would otherwise fail `Account::new` in words the classifier files as real.
fn l1_account() -> Account {
    dotenvy::dotenv().ok();
    if let Some(private_key) = std::env::var("POLYMARKET_PRIVATE_KEY")
        .ok()
        .filter(|key| !key.is_empty())
    {
        return Account::new(
            private_key,
            Credentials {
                key: String::new(),
                secret: String::new(),
                passphrase: String::new(),
            },
        )
        .expect("build account from private key");
    }
    #[cfg(feature = "keychain")]
    if let Ok(account) = Account::from_keychain() {
        return account;
    }
    panic!("POLYMARKET_PRIVATE_KEY required; the L2 triple is derived from it");
}

async fn derive_credentials() -> (String, String, String) {
    let account = l1_account();

    let clob = ClobBuilder::new()
        .with_account(account)
        .build()
        .expect("clob client");

    let resp = clob
        .auth()
        .expect("auth namespace")
        .derive_api_key(0)
        .send()
        .await
        .expect("derive_api_key should be accepted");

    (resp.api_key, resp.secret, resp.passphrase)
}

/// Open a raw user-channel socket, send `frame`, and report what the server did.
///
/// Deliberately raw rather than going through `UserSubscription`: the point is
/// to find out what the venue accepts *before* changing the SDK type, not to
/// confirm that a change we already made round-trips.
async fn probe_user_subscription(frame: String) -> Result<usize, String> {
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    // Going around the SDK also goes around its `ensure_crypto_provider` call,
    // and this graph enables two rustls backends — so without this the probe
    // panics before it reaches the network. Exactly the bug the SDK now fixes.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (mut ws, _) = connect_async("wss://ws-subscriptions-clob.polymarket.com/ws/user")
        .await
        .map_err(|e| format!("connect failed: {e}"))?;

    ws.send(Message::Text(frame.into()))
        .await
        .map_err(|e| format!("send failed: {e}"))?;

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
                return Err(format!("server closed the connection: {c:?}"));
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => return Err(format!("stream error: {e}")),
            Ok(None) => return Err("server ended the stream".to_string()),
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
        Err(e) => panic!(
            "server did NOT accept an omitted `markets` field: {e}\n\
             If this is reproducible, `markets` is genuinely required and the \
             AsyncAPI mirror is wrong — leave the SDK type as Vec<String> and \
             record the divergence."
        ),
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
    use polyoxide_clob::ws::ApiCredentials;

    let (api_key, secret, passphrase) = derive_credentials().await;
    let mut ws =
        WebSocket::connect_user_all_markets(ApiCredentials::new(api_key, secret, passphrase))
            .await
            .expect("unfiltered user subscription should connect");

    // No events are expected on an idle account; what matters is that the
    // server does not reject or close the subscription.
    match tokio::time::timeout(Duration::from_secs(20), ws.next()).await {
        Err(_) => println!("connection held open for 20s with no market filter"),
        Ok(Some(Ok(msg))) => println!("received a user event: {msg:?}"),
        Ok(Some(Err(e))) => panic!("server rejected the unfiltered subscription: {e}"),
        Ok(None) => panic!("server closed the unfiltered subscription"),
    }
}
