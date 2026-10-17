//! `Trading` on the CLOB against mock servers: the HTTP routes run on
//! mockito and the user socket on the kit's scripted server. The account is
//! Hardhat account 0, an EOA whose maker address is its own.

use std::borrow::Cow;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::clob::venue::{ClobOrderOptions, OrderExt};
use polyoxide_polymarket::clob::{
    Account, Clob, ClobBuilder, Credentials, SignatureType, TickSize,
};
use polyoxide_polymarket::data::DataApi;
use polyoxide_polymarket::shared::keys::{self, TokenId};
use polyoxide_venue::{
    Class, ClientOrderId, KeySpace, KillReason, MarketKey, OrderRequest, OrderStatus, ProductId,
    Side, TimeInForce, Trading, UnixMillis, VenueId,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

const TOKEN: &str = "100";
const MARKET: &str = "0xcondition";

/// Venue prose for a FAK order that found nothing to match against, verbatim.
const FAK_UNMATCHED_MSG: &str = "no orders found to match with FAK order. \
FAK orders are partially filled or killed if no match is found.";

/// Venue prose for a FOK order that could not be filled in full, verbatim.
const FOK_UNFILLED_MSG: &str =
    "order couldn't be fully filled. FOK orders are fully filled or killed.";

const LIVE: &str =
    r#"{"success":true,"orderID":"0xorder1","status":"live","transactionsHashes":[]}"#;
const MATCHED: &str =
    r#"{"success":true,"orderID":"0xorder1","status":"matched","transactionsHashes":["0xtx1"]}"#;

fn d(text: &str) -> Decimal {
    text.parse().expect("a decimal literal")
}

/// Hardhat account 0 with test credentials.
fn account() -> Account {
    let creds = Credentials {
        key: "test-key".into(),
        secret: "c2VjcmV0".into(), // base64("secret")
        passphrase: "test-pass".into(),
    };
    Account::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        creds,
    )
    .unwrap()
}

/// A CLOB client and a Data client, both on the one mock server.
fn client(server: &ServerGuard) -> Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .with_account(account())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .build()
        .unwrap()
}

/// A client that trades as a proxy wallet: built with `signature_type(PolyProxy)`
/// over an EOA account, as a live client for a proxy account is. With the
/// `gamma` feature it looks the proxy up on a Gamma client on the same server.
fn proxy_client(server: &ServerGuard) -> Clob {
    let builder = ClobBuilder::new()
        .base_url(server.url())
        .with_account(account())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .signature_type(SignatureType::PolyProxy);
    #[cfg(feature = "gamma")]
    let builder = builder.gamma(
        polyoxide_polymarket::gamma::Gamma::builder()
            .base_url(server.url())
            .build()
            .unwrap(),
    );
    builder.build().unwrap()
}

/// The proxy wallet the Gamma mock reports for the account. Digits only, so
/// its checksum and lowercase spellings are the same text.
#[cfg(feature = "gamma")]
const PROXY: &str = "0x1111111111111111111111111111111111111111";

/// `GET /public-profile` answering with a profile whose proxy wallet is
/// `PROXY`, once.
#[cfg(feature = "gamma")]
async fn proxy_profile(server: &mut ServerGuard) -> Mock {
    server
        .mock("GET", "/public-profile")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "proxyWallet": PROXY,
                "takerTier": 0,
                "takerTierName": "Tier 0",
                "weightedVolume": 0.0,
            })
            .to_string(),
        )
        .expect(1)
        .create_async()
        .await
}

fn key() -> MarketKey {
    keys::clob(&TokenId::new(TOKEN).unwrap())
}

fn buy(price: &str, size: &str) -> OrderRequest {
    OrderRequest::new(key(), Side::Buy, d(price), d(size))
}

fn with_options(mut request: OrderRequest, options: ClobOrderOptions) -> OrderRequest {
    request.extensions.insert(options).unwrap();
    request
}

/// Options that name the tick size and the exchange, so no lookup is sent.
#[allow(clippy::field_reassign_with_default)]
fn declared(tick: TickSize) -> ClobOrderOptions {
    let mut options = ClobOrderOptions::default();
    options.tick_size = Some(tick);
    options.neg_risk = Some(false);
    options
}

/// `/neg-risk` and `/tick-size`, which the client reads unless the options
/// name them. Each expects one request.
async fn lookups(server: &mut ServerGuard) -> (Mock, Mock) {
    let neg_risk = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .expect(1)
        .create_async()
        .await;
    let tick = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .expect(1)
        .create_async()
        .await;
    (neg_risk, tick)
}

/// `POST /order` answering 200 with `body`, `hits` times.
async fn post_ok(server: &mut ServerGuard, body: &str, hits: usize) -> Mock {
    server
        .mock("POST", "/order")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

/// `POST /order` refusing with 400 and the venue's prose, `hits` times.
async fn post_refused(server: &mut ServerGuard, message: &str, hits: usize) -> Mock {
    server
        .mock("POST", "/order")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(json!({ "error": message }).to_string())
        .expect(hits)
        .create_async()
        .await
}

/// A `GET /data/order/{id}` body: the shape `OpenOrder` decodes.
fn open_order(id: &str, status: &str, matched: &str) -> Value {
    json!({
        "id": id,
        "status": status,
        "owner": "test-key",
        "maker_address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "market": MARKET,
        "asset_id": TOKEN,
        "side": "BUY",
        "original_size": "10",
        "size_matched": matched,
        "price": "0.52",
        "outcome": "Yes",
        "expiration": "0",
        "order_type": "GTC",
        "associate_trades": ["trade-1"],
        "created_at": 1_792_000_000_i64,
    })
}

/// `GET /data/order/0xorder1` answering with `matched` shares matched.
async fn read_back(server: &mut ServerGuard, matched: &str) -> Mock {
    server
        .mock("GET", "/data/order/0xorder1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(open_order("0xorder1", "MATCHED", matched).to_string())
        .expect(1)
        .create_async()
        .await
}

/// Mocks that fail the test if any request reaches the server.
async fn nothing_sent(server: &mut ServerGuard) -> Vec<Mock> {
    let mut mocks = Vec::new();
    for method in ["GET", "POST", "DELETE"] {
        mocks.push(
            server
                .mock(method, Matcher::Any)
                .match_query(Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        );
    }
    mocks
}

/// A product of another venue, to hand the CLOB a key it does not own.
struct Elsewhere;

impl KeySpace for Elsewhere {
    const VENUE: VenueId = VenueId::from_static("elsewhere");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

/// An option type the CLOB does not declare.
#[derive(Debug, Clone, PartialEq)]
struct Stray;

// --- place ---

#[tokio::test]
async fn a_resting_post_only_order_is_open_and_carries_the_request() {
    let mut server = Server::new_async().await;
    let (neg_risk, tick) = lookups(&mut server).await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(
            json!({"owner": "test-key", "orderType": "GTC", "postOnly": true}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(LIVE)
        .expect(1)
        .create_async()
        .await;
    let mut request = buy("0.52", "10");
    request.post_only = true;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(order.id.as_str(), "0xorder1");
    assert_eq!(order.key, key());
    assert_eq!(order.side, Side::Buy);
    assert_eq!(order.price, d("0.52"));
    assert_eq!(order.size, d("10"));
    assert_eq!(order.filled, Decimal::ZERO);
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.time_in_force, Some(TimeInForce::GoodTilCancelled));
    let ext = order.extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.status.as_deref(), Some("live"));
    assert_eq!(ext.neg_risk, Some(false));
    neg_risk.assert_async().await;
    tick.assert_async().await;
    post.assert_async().await;
}

#[tokio::test]
async fn a_good_til_date_order_is_posted_with_its_expiration_in_whole_seconds() {
    let mut server = Server::new_async().await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(
            json!({"orderType": "GTD", "order": {"expiration": "1900000000"}}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(LIVE)
        .expect(1)
        .create_async()
        .await;
    let mut options = declared(TickSize::Hundredth);
    options.expiration = Some(UnixMillis(1_900_000_000_999));

    let order = client(&server)
        .place(with_options(buy("0.52", "10"), options))
        .await
        .unwrap();

    assert_eq!(order.status, OrderStatus::Open);
    let ext = order.extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.expiration, Some(UnixMillis(1_900_000_000_999)));
    post.assert_async().await;
}

#[tokio::test]
async fn a_fak_kill_is_ok_and_killed_unmatched() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_refused(&mut server, FAK_UNMATCHED_MSG, 1).await;
    let mut request = buy("0.01", "100");
    request.time_in_force = TimeInForce::ImmediateOrCancel;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert_eq!(order.filled, Decimal::ZERO);
    // The id is the order's EIP-712 hash, computed before sending.
    assert!(order.id.as_str().starts_with("0x"));
    assert_eq!(order.id.as_str().len(), 66);
    assert_eq!(order.time_in_force, Some(TimeInForce::ImmediateOrCancel));
    post.assert_async().await;
}

#[tokio::test]
async fn a_fok_kill_is_ok_and_killed_unfillable() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_refused(&mut server, FOK_UNFILLED_MSG, 1).await;
    let mut request = buy("0.01", "100");
    request.time_in_force = TimeInForce::FillOrKill;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    );
    assert_eq!(order.filled, Decimal::ZERO);
    post.assert_async().await;
}

#[tokio::test]
async fn a_matched_order_is_read_back_as_filled() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_ok(&mut server, MATCHED, 1).await;
    let read = read_back(&mut server, "10").await;

    let order = client(&server).place(buy("0.52", "10")).await.unwrap();

    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(order.filled, d("10"));
    let ext = order.extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.status.as_deref(), Some("matched"));
    assert_eq!(ext.transaction_hashes, ["0xtx1"]);
    post.assert_async().await;
    read.assert_async().await;
}

#[tokio::test]
async fn an_ioc_partial_match_is_killed_unmatched_with_what_matched() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(json!({"orderType": "FAK"})))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(MATCHED)
        .expect(1)
        .create_async()
        .await;
    let _read = read_back(&mut server, "4").await;
    let mut request = buy("0.52", "10");
    request.time_in_force = TimeInForce::ImmediateOrCancel;

    let order = client(&server).place(request).await.unwrap();

    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert_eq!(order.filled, d("4"));
    post.assert_async().await;
}

#[tokio::test]
async fn a_matched_order_whose_read_back_fails_is_reported_open() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let _post = post_ok(&mut server, MATCHED, 1).await;
    // A 500 is not retried, so the read-back fails at once.
    let read = server
        .mock("GET", "/data/order/0xorder1")
        .with_status(500)
        .with_body("upstream down")
        .expect(1)
        .create_async()
        .await;

    // The order exists on the venue, so an error here would invite a duplicate.
    let order = client(&server).place(buy("0.52", "10")).await.unwrap();

    assert_eq!(order.id.as_str(), "0xorder1");
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.filled, Decimal::ZERO);
    read.assert_async().await;
}

#[tokio::test]
async fn a_post_the_clob_refuses_is_a_venue_refusal_carrying_its_message() {
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let post = post_ok(
        &mut server,
        r#"{"success":false,"errorMsg":"not enough balance / allowance"}"#,
        1,
    )
    .await;

    let err = client(&server).place(buy("0.52", "10")).await.unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(err.to_string().contains("not enough balance"), "{err}");
    post.assert_async().await;
}

#[tokio::test]
async fn the_tick_size_and_neg_risk_are_looked_up_only_when_the_options_lack_them() {
    let mut server = Server::new_async().await;
    let (neg_risk, tick) = lookups(&mut server).await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let clob = client(&server);

    clob.place(buy("0.52", "10")).await.unwrap();
    clob.place(with_options(
        buy("0.52", "10"),
        declared(TickSize::Hundredth),
    ))
    .await
    .unwrap();

    // One lookup each, from the first order alone.
    neg_risk.assert_async().await;
    tick.assert_async().await;
    post.assert_async().await;
}

#[tokio::test]
async fn requests_the_clob_cannot_honour_are_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let untouched = nothing_sent(&mut server).await;
    let clob = client(&server);
    let tick = declared(TickSize::Hundredth);

    let mut reduce_only = buy("0.52", "10");
    reduce_only.reduce_only = true;
    let mut post_only_ioc = buy("0.52", "10");
    post_only_ioc.post_only = true;
    post_only_ioc.time_in_force = TimeInForce::ImmediateOrCancel;
    let mut expiring = declared(TickSize::Hundredth);
    expiring.expiration = Some(UnixMillis(1_900_000_000_000));
    let mut expiring_fok = with_options(buy("0.52", "10"), expiring);
    expiring_fok.time_in_force = TimeInForce::FillOrKill;
    let mut stray = buy("0.52", "10");
    stray.extensions.insert(Stray).unwrap();
    let foreign = OrderRequest::new(
        Elsewhere::key(&"1".to_owned()),
        Side::Buy,
        d("0.52"),
        d("10"),
    );

    let cases = [
        ("reduce-only", reduce_only, "reduce-only"),
        ("post-only with IOC", post_only_ioc, "post-only"),
        ("expiration with FOK", expiring_fok, "expiration"),
        ("an undeclared extension", stray, "does not declare"),
        ("a key of another product", foreign, ""),
        (
            "an off-tick price",
            with_options(buy("0.525", "10"), tick),
            "tick size",
        ),
        (
            "a price of one",
            with_options(buy("1", "10"), tick),
            "strictly between",
        ),
        (
            "a size of three decimals",
            with_options(buy("0.52", "5.001"), tick),
            "decimal places",
        ),
        (
            "a size below the minimum",
            with_options(buy("0.52", "0.00"), tick),
            "smallest order",
        ),
    ];
    for (label, request, expected) in cases {
        let err = clob.place(request).await.unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest, "{label}: {err}");
        assert!(err.to_string().contains(expected), "{label}: {err}");
    }

    for mock in untouched {
        mock.assert_async().await;
    }
}

#[tokio::test]
async fn a_client_order_id_in_use_is_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 1).await;
    let clob = client(&server);
    let mut first = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    first.client_order_id = Some(ClientOrderId::new("mine-1"));
    let mut second = first.clone();
    second.price = d("0.53");

    let order = clob.place(first).await.unwrap();
    let err = clob.place(second).await.unwrap_err();

    assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine-1")));
    assert_eq!(err.class, Class::InvalidRequest);
    assert!(
        err.to_string()
            .contains("client order id `mine-1` is already in use by order `0xorder1`"),
        "{err}"
    );
    // The first order alone was posted.
    post.assert_async().await;
}

#[tokio::test]
async fn a_killed_order_frees_its_client_order_id() {
    let mut server = Server::new_async().await;
    let post = post_refused(&mut server, FAK_UNMATCHED_MSG, 2).await;
    let clob = client(&server);
    let mut request = with_options(buy("0.01", "100"), declared(TickSize::Hundredth));
    request.time_in_force = TimeInForce::ImmediateOrCancel;
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    clob.place(request.clone()).await.unwrap();
    // The kill ended the order, so its client id is free again.
    let order = clob.place(request).await.unwrap();

    assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine-1")));
    post.assert_async().await;
}

#[tokio::test]
async fn a_client_without_an_account_refuses_to_place() {
    let err = Clob::public().place(buy("0.52", "10")).await.unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest);
    assert!(err.to_string().contains("Account required"), "{err}");
}

/// `place` signs with the client's signature type, not the account target's:
/// this client is built for a proxy wallet, and without `gamma` (which `clob`
/// does not imply) it has no way to find that wallet, so the native refusal of
/// a proxy order with no funder comes back and nothing is sent. A `place` that
/// left the type to the account's EOA target would post from the EOA instead.
#[cfg(not(feature = "gamma"))]
#[tokio::test]
async fn a_proxy_client_without_gamma_refuses_to_place_and_sends_nothing() {
    let mut server = Server::new_async().await;
    let untouched = nothing_sent(&mut server).await;
    let request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));

    let err = proxy_client(&server).place(request).await.unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(
        err.to_string().contains("requires the `gamma` feature"),
        "{err}"
    );
    for mock in untouched {
        mock.assert_async().await;
    }
}

/// With `gamma`, the same client posts the order from the proxy wallet Gamma
/// reports, as a proxy order (signature type 1), and not from the EOA.
#[cfg(feature = "gamma")]
#[tokio::test]
async fn a_proxy_client_places_from_its_proxy_wallet() {
    let mut server = Server::new_async().await;
    let profile = proxy_profile(&mut server).await;
    let post = server
        .mock("POST", "/order")
        .match_body(Matcher::PartialJson(
            json!({"order": {"maker": PROXY, "signatureType": 1}}),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(LIVE)
        .expect(1)
        .create_async()
        .await;
    let request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));

    let order = proxy_client(&server).place(request).await.unwrap();

    assert_eq!(order.status, OrderStatus::Open);
    profile.assert_async().await;
    post.assert_async().await;
}
