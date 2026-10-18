//! `Trading` on the CLOB against mock servers: the HTTP routes run on
//! mockito and the user socket on the kit's scripted server. The account is
//! Hardhat account 0, an EOA whose maker address is its own.

use std::borrow::Cow;

use futures_util::StreamExt;
use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::clob::venue::{BalanceExt, PositionExt};
use polyoxide_polymarket::clob::venue::{ClobOrderOptions, OrderExt};
use polyoxide_polymarket::clob::ws::UserWsBuilder;
use polyoxide_polymarket::clob::{
    Account, Clob, ClobBuilder, Credentials, SignatureType, TickSize,
};
use polyoxide_polymarket::data::DataApi;
use polyoxide_polymarket::shared::keys::{self, TokenId};
use polyoxide_venue::OrderId;
use polyoxide_venue::{
    Class, ClientOrderId, KeySpace, KillReason, MarketKey, OrderRequest, OrderStatus, ProductId,
    Side, TimeInForce, Trading, UnixMillis, VenueId,
};
use polyoxide_venue::{DynTrading, Fill, Liquidity, TradingEvent};
use polyoxide_ws::test_server::{next_item, Script, ScriptedServer};
use polyoxide_ws::tungstenite::Message;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

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

// --- events ---

const COMPLEMENT: &str = "101";

/// The account's maker address: Hardhat account 0 is an EOA.
fn ours() -> String {
    account().address().to_string()
}

/// A client whose user socket is `ws`, quick to ping and to reconnect.
fn client_on(server: &ServerGuard, ws: &ScriptedServer) -> Clob {
    let socket = UserWsBuilder::new()
        .url(ws.url.clone())
        .unwrap()
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_secs(2))
        .backoff(Duration::from_millis(10), Duration::from_millis(20));
    ClobBuilder::new()
        .base_url(server.url())
        .with_account(account())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .user_socket(socket)
        .build()
        .unwrap()
}

/// Answers the client's text `PING` with a `PONG`, as the CLOB does. An
/// integration test cannot see the crate's `#[cfg(test)]` answerer.
fn pong(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send + 'static {
    |text: &str| {
        if text == "PING" {
            vec![Message::Text("PONG".into())]
        } else {
            Vec::new()
        }
    }
}

async fn user_socket(scripts: Vec<Script>) -> ScriptedServer {
    ScriptedServer::start_answering(scripts, pong).await
}

fn order_frame(id: &str, kind: &str, matched: &str, timestamp: &str) -> String {
    json!({
        "event_type": "order",
        "id": id,
        "asset_id": TOKEN,
        "market": MARKET,
        "outcome": "Yes",
        "price": "0.52",
        "side": "BUY",
        "original_size": "10",
        "size_matched": matched,
        "type": kind,
        "order_owner": "test-key",
        "timestamp": timestamp
    })
    .to_string()
}

fn maker_entry(order: &str, address: &str, asset: &str, price: &str) -> Value {
    json!({
        "order_id": order,
        "maker_address": address,
        "matched_amount": "4",
        "fee_rate_bps": "0",
        "asset_id": asset,
        "price": price
    })
}

fn trade_frame(
    id: &str,
    status: &str,
    owner: &str,
    taker_order: &str,
    makers: Vec<Value>,
) -> String {
    json!({
        "event_type": "trade",
        "id": id,
        "asset_id": TOKEN,
        "market": MARKET,
        "outcome": "Yes",
        "price": "0.52",
        "size": "4",
        "side": "BUY",
        "status": status,
        "taker_order_id": taker_order,
        "maker_orders": makers,
        "owner": owner,
        "transaction_hash": "0xhash",
        "timestamp": "1792000000123"
    })
    .to_string()
}

/// A trade the account took, alone on the book.
fn taker_trade(id: &str) -> String {
    trade_frame(id, "MATCHED", "test-key", "0xtaker", vec![])
}

fn fill_of(event: TradingEvent) -> Fill {
    match event {
        TradingEvent::Fill(fill) => fill,
        other => panic!("expected a fill, got {other:?}"),
    }
}

#[tokio::test]
async fn the_stream_leads_with_a_resync_and_acks_with_the_client_order_id() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let placement = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let cancellation = order_frame("0xorder1", "CANCELLATION", "0", "1792000009999");
    let ws = user_socket(vec![Script::frames(&[
        placement.as_str(),
        cancellation.as_str(),
    ])
    .after_first_frame()])
    .await;
    let clob = client_on(&server, &ws);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));
    // The order is placed before the stream is polled, so the ledger holds
    // its client id when the acks arrive.
    clob.place(request.clone()).await.unwrap();

    let mut events = clob.events();
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let TradingEvent::Ack(placed) = next_item(&mut events).await.unwrap() else {
        panic!("expected the placement's ack");
    };
    let TradingEvent::Ack(cancelled) = next_item(&mut events).await.unwrap() else {
        panic!("expected the cancellation's ack");
    };

    assert_eq!(placed.id.as_str(), "0xorder1");
    assert_eq!(placed.status, OrderStatus::Open);
    assert_eq!(placed.key, key());
    assert_eq!(placed.side, Side::Buy);
    assert_eq!(placed.price, d("0.52"));
    assert_eq!(placed.size, d("10"));
    assert_eq!(placed.created, Some(UnixMillis(1_792_000_000_123)));
    assert_eq!(placed.client_order_id, Some(ClientOrderId::new("mine-1")));
    assert_eq!(cancelled.status, OrderStatus::Cancelled);
    assert_eq!(
        cancelled.client_order_id,
        Some(ClientOrderId::new("mine-1"))
    );
    // The socket was started with the account's credentials and on every
    // market: the subscribe frame carries the key and no market filter.
    ws.wait_for("the subscription", |ws| !ws.first_frames().is_empty())
        .await;
    let subscription = ws.first_frames()[0].clone();
    assert!(subscription.contains("test-key"), "{subscription}");
    assert!(!subscription.contains("\"markets\""), "{subscription}");
    // The cancelled ack dropped the reuse guard, so the id is free again.
    clob.place(request).await.unwrap();
    post.assert_async().await;
}

#[tokio::test]
async fn an_order_cancelled_before_the_stream_is_polled_still_acks_with_its_client_order_id() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xorder1"}),
        r#"{"canceled":["0xorder1"],"notCanceled":{}}"#,
    )
    .await;
    let placement = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let cancellation = order_frame("0xorder1", "CANCELLATION", "0", "1792000009999");
    let ws = user_socket(vec![Script::frames(&[
        placement.as_str(),
        cancellation.as_str(),
    ])
    .after_first_frame()])
    .await;
    let clob = client_on(&server, &ws);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    clob.place(request.clone()).await.unwrap();
    clob.cancel(&OrderId::new("0xorder1")).await.unwrap();
    let mut events = clob.events();
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let TradingEvent::Ack(placed) = next_item(&mut events).await.unwrap() else {
        panic!("expected the placement's ack");
    };
    let TradingEvent::Ack(cancelled) = next_item(&mut events).await.unwrap() else {
        panic!("expected the cancellation's ack");
    };

    assert_eq!(placed.client_order_id, Some(ClientOrderId::new("mine-1")));
    assert_eq!(cancelled.status, OrderStatus::Cancelled);
    assert_eq!(
        cancelled.client_order_id,
        Some(ClientOrderId::new("mine-1"))
    );
    // Both acks carried the id, and it is free to place again.
    let again = clob.place(request).await.unwrap();
    assert_eq!(again.client_order_id, Some(ClientOrderId::new("mine-1")));
    post.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn a_taker_fill_and_two_maker_fills_name_the_orders_of_the_account() {
    let server = Server::new_async().await;
    // The account took, and no maker entry is its own.
    let one = trade_frame(
        "t1",
        "MATCHED",
        "test-key",
        "0xtaker",
        vec![maker_entry(
            "0xm1",
            "0xdead000000000000000000000000000000000002",
            TOKEN,
            "0.52",
        )],
    );
    // Someone else took; the account rests on the complementary token, its
    // address in upper case.
    let mint = trade_frame(
        "t1b",
        "MATCHED",
        "other-key",
        "0xother",
        vec![
            maker_entry(
                "0xm1",
                "0xdead000000000000000000000000000000000002",
                TOKEN,
                "0.52",
            ),
            maker_entry(
                "0xm2",
                &ours().to_uppercase().replacen("0X", "0x", 1),
                COMPLEMENT,
                "0.48",
            ),
        ],
    );
    // Someone else took; the account's order rests on the trade's own token.
    let two = trade_frame(
        "t2",
        "MATCHED",
        "other-key",
        "0xother",
        vec![maker_entry("0xm3", &ours().to_lowercase(), TOKEN, "0.52")],
    );
    let ws = user_socket(vec![Script::frames(&[
        one.as_str(),
        mint.as_str(),
        two.as_str(),
    ])
    .after_first_frame()])
    .await;
    let mut events = client_on(&server, &ws).events();

    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let taker = fill_of(next_item(&mut events).await.unwrap());
    let complement = fill_of(next_item(&mut events).await.unwrap());
    let resting = fill_of(next_item(&mut events).await.unwrap());

    assert_eq!(taker.id.as_str(), "t1:0xtaker");
    assert_eq!(taker.liquidity, Some(Liquidity::Taker));
    assert_eq!(taker.side, Side::Buy);
    assert_eq!(taker.key, key());
    assert_eq!(taker.price, d("0.52"));
    assert_eq!(taker.size, d("4"));
    assert_eq!(taker.time, UnixMillis(1_792_000_000_123));
    assert_eq!(taker.trade_id.as_ref().map(|id| id.as_str()), Some("t1"));
    assert_eq!(complement.id.as_str(), "t1b:0xm2");
    assert_eq!(complement.liquidity, Some(Liquidity::Maker));
    assert_eq!(
        complement.key,
        keys::clob(&TokenId::new(COMPLEMENT).unwrap())
    );
    assert_eq!(complement.side, Side::Buy);
    assert_eq!(complement.price, d("0.48"));
    assert_eq!(resting.id.as_str(), "t2:0xm3");
    assert_eq!(resting.liquidity, Some(Liquidity::Maker));
    assert_eq!(resting.side, Side::Sell);
}

#[tokio::test]
async fn a_self_trade_yields_only_its_maker_fill_and_the_stream_goes_on() {
    let server = Server::new_async().await;
    // The account's key is the trade's `owner`, and one maker entry is the
    // account's own: D17 reads that as no taker fill.
    let own = trade_frame(
        "t1",
        "MATCHED",
        "test-key",
        "0xtaker",
        vec![maker_entry("0xm1", &ours(), TOKEN, "0.52")],
    );
    let ack = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![
        Script::frames(&[own.as_str(), ack.as_str()]).after_first_frame()
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let only = fill_of(next_item(&mut events).await.unwrap());

    assert_eq!(only.id.as_str(), "t1:0xm1");
    assert_eq!(only.liquidity, Some(Liquidity::Maker));
    // The next event is the ack, so no taker fill sat between them.
    assert!(matches!(
        next_item(&mut events).await.unwrap(),
        TradingEvent::Ack(_)
    ));
}

#[tokio::test]
async fn a_failed_trade_is_a_resync_and_a_mined_one_is_silent() {
    let server = Server::new_async().await;
    let mined = trade_frame("t1", "MINED", "test-key", "0xtaker", vec![]);
    let failed = trade_frame("t1", "FAILED", "test-key", "0xtaker", vec![]);
    let ack = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![Script::frames(&[
        mined.as_str(),
        failed.as_str(),
        ack.as_str(),
    ])
    .after_first_frame()])
    .await;
    let mut events = client_on(&server, &ws).events();

    // The leading one, then the failed trade's; the mined trade yields nothing.
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    assert!(matches!(
        next_item(&mut events).await.unwrap(),
        TradingEvent::Ack(_)
    ));
}

#[tokio::test]
async fn a_trade_naming_none_of_the_accounts_orders_is_an_error_item_and_the_stream_goes_on() {
    let server = Server::new_async().await;
    let stranger = trade_frame(
        "t1",
        "MATCHED",
        "other-key",
        "0xother",
        vec![maker_entry(
            "0xm1",
            "0xdead000000000000000000000000000000000002",
            TOKEN,
            "0.52",
        )],
    );
    let ack = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![
        Script::frames(&[stranger.as_str(), ack.as_str()]).after_first_frame()
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let err = next_item(&mut events).await.unwrap_err();
    assert_eq!(err.class, Class::Decode, "{err}");
    assert!(matches!(
        next_item(&mut events).await.unwrap(),
        TradingEvent::Ack(_)
    ));
}

#[tokio::test]
async fn every_fill_arrives_in_order_across_a_slow_consumer_and_a_server_cut() {
    let server = Server::new_async().await;
    let before: Vec<String> = (1..=3).map(|n| taker_trade(&format!("t{n}"))).collect();
    let before: Vec<&str> = before.iter().map(String::as_str).collect();
    let after = taker_trade("t4");
    let ws = user_socket(vec![
        // The first connection pushes three fills and is cut with no close
        // frame; the next one pushes the fourth.
        Script::frames(&before).after_first_frame().then_cut(),
        Script::frames(&[after.as_str()]).after_first_frame(),
    ])
    .await;
    let mut events = client_on(&server, &ws).events();

    let mut seen = Vec::new();
    let mut fills = 0;
    while fills < 4 {
        // The consumer is slower than the server; the socket's queue is
        // unbounded, so nothing is lost.
        tokio::time::sleep(Duration::from_millis(40)).await;
        match next_item(&mut events).await {
            Ok(TradingEvent::Fill(fill)) => {
                fills += 1;
                seen.push(fill.id.to_string());
            }
            Ok(TradingEvent::Resync) => seen.push("resync".to_owned()),
            Ok(other) => panic!("unexpected {other:?}"),
            // The cut is reported as an error item first.
            Err(_) => {}
        }
    }

    assert_eq!(
        seen,
        [
            "resync",
            "t1:0xtaker",
            "t2:0xtaker",
            "t3:0xtaker",
            "resync",
            "t4:0xtaker"
        ]
    );
}

#[tokio::test]
async fn events_without_an_account_yield_its_error_and_end() {
    let mut events = Clob::public().events();

    let err = events.next().await.unwrap().unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(err.to_string().contains("Account required"), "{err}");
    assert!(events.next().await.is_none());
}

/// The stream follows the maker address `place` trades from. A proxy client
/// without `gamma` cannot find its proxy wallet, so the stream's only item is
/// the native refusal and it opens no socket (the default socket is the CLOB's
/// live host, which a test must never reach); a stream that resolved the
/// account target's EOA would go on to connect.
#[cfg(not(feature = "gamma"))]
#[tokio::test]
async fn a_proxy_client_without_gamma_yields_the_refusal_and_ends() {
    let server = Server::new_async().await;
    let mut events = proxy_client(&server).events();

    let err = events.next().await.unwrap().unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(
        err.to_string().contains("requires the `gamma` feature"),
        "{err}"
    );
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn a_clob_runs_behind_an_arc_of_dyn_trading_and_its_stream_outlives_the_arc() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 1).await;
    let placement = order_frame("0xorder1", "PLACEMENT", "0", "1792000000123");
    let ws = user_socket(vec![
        Script::frames(&[placement.as_str()]).after_first_frame()
    ])
    .await;
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(client_on(&server, &ws));

    let order = venue
        .place(with_options(
            buy("0.52", "10"),
            declared(TickSize::Hundredth),
        ))
        .await
        .unwrap();
    let mut events = venue.events();
    drop(venue);

    assert_eq!(order.id.as_str(), "0xorder1");
    assert_eq!(next_item(&mut events).await.unwrap(), TradingEvent::Resync);
    let TradingEvent::Ack(acked) = next_item(&mut events).await.unwrap() else {
        panic!("expected the placement's ack");
    };
    assert_eq!(acked.id, order.id);
    post.assert_async().await;
}

// --- cancel, orders, positions and balances ---

/// A `DELETE` answering with a `BatchCancelResponse` body.
async fn delete_answering(server: &mut ServerGuard, path: &str, body: Value, answer: &str) -> Mock {
    server
        .mock("DELETE", path)
        .match_body(Matcher::Json(body))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(answer)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn cancel_is_ok_when_the_order_is_among_the_canceled() {
    let mut server = Server::new_async().await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xa"}),
        r#"{"canceled":["0xa"],"notCanceled":{}}"#,
    )
    .await;

    client(&server).cancel(&OrderId::new("0xa")).await.unwrap();

    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_is_a_venue_refusal_carrying_the_venues_reason() {
    let mut server = Server::new_async().await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xa"}),
        r#"{"canceled":[],"notCanceled":{"0xa":"order can't be found - already canceled or matched"}}"#,
    )
    .await;

    let err = client(&server)
        .cancel(&OrderId::new("0xa"))
        .await
        .unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(
        err.to_string().contains("already canceled or matched"),
        "{err}"
    );
    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_of_an_order_the_clob_does_not_answer_for_is_a_venue_refusal() {
    let mut server = Server::new_async().await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xa"}),
        r#"{"canceled":[],"notCanceled":{}}"#,
    )
    .await;

    let err = client(&server)
        .cancel(&OrderId::new("0xa"))
        .await
        .unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("the CLOB did not answer for order 0xa"),
        "{err}"
    );
    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_all_returns_only_the_ids_the_clob_cancelled() {
    let mut server = Server::new_async().await;
    let delete = server
        .mock("DELETE", "/cancel-all")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled":["0xa","0xb"],"notCanceled":{"0xc":"order can't be found"}}"#)
        .expect(1)
        .create_async()
        .await;

    let ids = client(&server).cancel_all(None).await.unwrap();

    // `0xc` stays open: it is in `notCanceled`.
    assert_eq!(ids, [OrderId::new("0xa"), OrderId::new("0xb")]);
    delete.assert_async().await;
}

#[tokio::test]
async fn cancel_all_for_a_key_resolves_the_market_and_names_both_it_and_the_token() {
    let mut server = Server::new_async().await;
    let lookup = server
        .mock("GET", "/markets-by-token/100")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"condition_id":"0xcond","primary_token_id":"100","secondary_token_id":"101"}"#,
        )
        .expect(1)
        .create_async()
        .await;
    let delete = delete_answering(
        &mut server,
        "/cancel-market-orders",
        json!({"market": "0xcond", "asset_id": "100"}),
        r#"{"canceled":["0xa"],"notCanceled":{}}"#,
    )
    .await;

    let ids = client(&server).cancel_all(Some(&key())).await.unwrap();

    assert_eq!(ids, [OrderId::new("0xa")]);
    lookup.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn cancelling_an_order_frees_the_client_order_id_the_ledger_held() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let delete = server
        .mock("DELETE", "/cancel-all")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled":["0xorder1"],"notCanceled":{}}"#)
        .expect(1)
        .create_async()
        .await;
    let clob = client(&server);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    clob.place(request.clone()).await.unwrap();
    let refused = clob.place(request.clone()).await.unwrap_err();
    clob.cancel_all(None).await.unwrap();
    let again = clob.place(request).await.unwrap();

    assert!(refused.to_string().contains("already in use"), "{refused}");
    assert_eq!(again.client_order_id, Some(ClientOrderId::new("mine-1")));
    post.assert_async().await;
    delete.assert_async().await;
}

/// A `/data/orders` body: `rows` and the cursor to the next page.
fn orders_page(rows: Vec<Value>, next_cursor: &str) -> String {
    json!({"data": rows, "next_cursor": next_cursor}).to_string()
}

#[tokio::test]
async fn open_orders_pages_to_the_end_sentinel() {
    let mut server = Server::new_async().await;
    let first = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xa", "LIVE", "0")], "MQ=="))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::UrlEncoded("next_cursor".into(), "MQ==".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(
            vec![open_order("0xb", "MATCHED", "10")],
            "LTE=",
        ))
        .expect(1)
        .create_async()
        .await;

    let orders = client(&server).open_orders(None).await.unwrap();

    let ids: Vec<&str> = orders.iter().map(|order| order.id.as_str()).collect();
    assert_eq!(ids, ["0xa", "0xb"]);
    assert_eq!(orders[0].status, OrderStatus::Open);
    assert_eq!(orders[1].status, OrderStatus::Filled);
    assert_eq!(orders[1].filled, d("10"));
    assert_eq!(orders[0].key, key());
    assert_eq!(orders[0].price, d("0.52"));
    assert_eq!(orders[0].time_in_force, Some(TimeInForce::GoodTilCancelled));
    assert_eq!(orders[0].created, Some(UnixMillis(1_792_000_000_000)));
    let ext = orders[0].extensions.get::<OrderExt>().unwrap();
    assert_eq!(ext.status.as_deref(), Some("LIVE"));
    assert_eq!(ext.condition_id.as_deref(), Some(MARKET));
    assert_eq!(ext.trade_ids, ["trade-1"]);
    first.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn open_orders_of_a_key_filters_by_its_token() {
    let mut server = Server::new_async().await;
    let list = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::UrlEncoded("asset_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xa", "LIVE", "0")], "LTE="))
        .expect(1)
        .create_async()
        .await;

    let orders = client(&server).open_orders(Some(&key())).await.unwrap();

    assert_eq!(orders.len(), 1);
    list.assert_async().await;
}

#[tokio::test]
async fn open_orders_stops_on_a_cursor_it_has_already_followed() {
    let mut server = Server::new_async().await;
    // Every page names the same next cursor.
    let list = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(orders_page(vec![open_order("0xa", "LIVE", "0")], "MQ=="))
        .expect(2)
        .create_async()
        .await;

    let err = client(&server).open_orders(None).await.unwrap_err();

    assert_eq!(err.class, Class::Decode, "{err}");
    list.assert_async().await;
}

/// A `/positions` row with the asset `asset`.
fn position_row(asset: String) -> Value {
    json!({
        "proxyWallet": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "asset": asset,
        "conditionId": MARKET,
        "size": 12.5,
        "avgPrice": 0.4,
        "initialValue": 5.0,
        "currentValue": 6.25,
        "cashPnl": 1.25,
        "percentPnl": 25.0,
        "totalBought": 12.5,
        "realizedPnl": 0.0,
        "percentRealizedPnl": 0.0,
        "curPrice": 0.5,
        "redeemable": false,
        "mergeable": false,
        "title": "A market",
        "slug": "a-market",
        "icon": null,
        "eventSlug": null,
        "outcome": "Yes",
        "outcomeIndex": 0,
        "oppositeOutcome": "No",
        "oppositeAsset": "101",
        "endDate": null,
        "negativeRisk": false
    })
}

/// The query of one `/positions` page: the maker, no dust filter, 500 rows.
fn positions_query(user: &str, offset: &str) -> Matcher {
    Matcher::AllOf(vec![
        Matcher::UrlEncoded("user".into(), user.into()),
        Matcher::UrlEncoded("sizeThreshold".into(), "0".into()),
        Matcher::UrlEncoded("limit".into(), "500".into()),
        Matcher::UrlEncoded("offset".into(), offset.into()),
    ])
}

fn position_rows(count: usize) -> String {
    let rows: Vec<Value> = (0..count)
        .map(|n| position_row((1_000 + n).to_string()))
        .collect();
    Value::Array(rows).to_string()
}

#[tokio::test]
async fn positions_pages_by_offset_with_no_dust_filter_for_the_maker_address() {
    let mut server = Server::new_async().await;
    let clob = client(&server);
    let user = clob.account().unwrap().address().to_string();
    let first = server
        .mock("GET", "/positions")
        .match_query(positions_query(&user, "0"))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(500))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("GET", "/positions")
        .match_query(positions_query(&user, "500"))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(3))
        .expect(1)
        .create_async()
        .await;

    let positions = clob.positions().await.unwrap();

    assert_eq!(positions.len(), 503);
    assert_eq!(positions[0].key, keys::clob(&TokenId::new("1000").unwrap()));
    assert_eq!(positions[0].size, d("12.5"));
    assert_eq!(positions[0].average_price, Some(d("0.4")));
    let ext = positions[0].extensions.get::<PositionExt>().unwrap();
    assert_eq!(ext.condition_id.as_deref(), Some(MARKET));
    assert_eq!(ext.current_price, Some(d("0.5")));
    assert_eq!(ext.outcome_index, Some(0));
    assert_eq!(ext.negative_risk, Some(false));
    first.assert_async().await;
    second.assert_async().await;
}

/// `positions` reads the address `place` trades from, the client's signature
/// type's. Without `gamma` a proxy client cannot find its proxy wallet, so the
/// native refusal comes back and nothing is sent; a `positions` that resolved
/// from the account target's EOA would list the EOA's rows instead.
#[cfg(not(feature = "gamma"))]
#[tokio::test]
async fn a_proxy_client_without_gamma_refuses_to_list_positions_and_sends_nothing() {
    let mut server = Server::new_async().await;
    let untouched = nothing_sent(&mut server).await;

    let err = proxy_client(&server).positions().await.unwrap_err();

    assert_eq!(err.class, Class::InvalidRequest, "{err}");
    assert!(
        err.to_string().contains("requires the `gamma` feature"),
        "{err}"
    );
    for mock in untouched {
        mock.assert_async().await;
    }
}

/// With `gamma`, the proxy client lists the positions of its proxy wallet.
#[cfg(feature = "gamma")]
#[tokio::test]
async fn a_proxy_client_lists_the_positions_of_its_proxy_wallet() {
    let mut server = Server::new_async().await;
    let profile = proxy_profile(&mut server).await;
    let list = server
        .mock("GET", "/positions")
        .match_query(positions_query(PROXY, "0"))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(2))
        .expect(1)
        .create_async()
        .await;

    let positions = proxy_client(&server).positions().await.unwrap();

    assert_eq!(positions.len(), 2);
    profile.assert_async().await;
    list.assert_async().await;
}

#[tokio::test]
async fn positions_refuse_a_wallet_with_more_rows_than_the_offset_reaches() {
    let mut server = Server::new_async().await;
    // Every page is full: offsets 0 to 10000 are 21 pages, and a 22nd would
    // need offset 10500.
    let pages = server
        .mock("GET", "/positions")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_rows(500))
        .expect(21)
        .create_async()
        .await;

    let err = client(&server).positions().await.unwrap_err();

    assert!(matches!(err.class, Class::VenueRefusal { .. }), "{err}");
    assert!(err.to_string().contains("more positions"), "{err}");
    pages.assert_async().await;
}

#[tokio::test]
async fn balances_are_scaled_from_six_decimals_and_name_every_allowance() {
    let mut server = Server::new_async().await;
    let balance = server
        .mock("GET", "/balance-allowance")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("asset_type".into(), "COLLATERAL".into()),
            Matcher::UrlEncoded("signature_type".into(), "0".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "balance": "141171137",
                "allowances": {
                    "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E": "999999",
                    "0xC5d563A36AE78145C45a50134d48A1215220f80a":
                        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
                }
            })
            .to_string(),
        )
        .expect(1)
        .create_async()
        .await;

    let balances = client(&server).balances().await.unwrap();

    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0].asset, "pUSD");
    assert_eq!(balances[0].total, Some(d("141.171137")));
    assert_eq!(balances[0].available, None);
    let ext = balances[0].extensions.get::<BalanceExt>().unwrap();
    assert_eq!(
        ext.allowances["0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"],
        d("0.999999")
    );
    // An unlimited approval is larger than a decimal holds.
    assert_eq!(
        ext.allowances["0xC5d563A36AE78145C45a50134d48A1215220f80a"],
        Decimal::MAX
    );
    balance.assert_async().await;
}

#[tokio::test]
async fn every_read_needs_an_account() {
    let clob = Clob::public();

    let errs = [
        clob.cancel(&OrderId::new("0xa")).await.unwrap_err(),
        clob.cancel_all(None).await.unwrap_err(),
        clob.open_orders(None).await.unwrap_err(),
        clob.positions().await.unwrap_err(),
        clob.balances().await.unwrap_err(),
    ];

    for err in errs {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
        assert!(err.to_string().contains("Account required"), "{err}");
    }
}

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
async fn a_post_whose_body_does_not_decode_keeps_its_client_order_id_held() {
    let mut server = Server::new_async().await;
    // The venue may have taken the order: a 2xx whose body is not an answer.
    let post = post_ok(&mut server, r#"{"unexpected": true}"#, 1).await;
    let clob = client(&server);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    let err = clob.place(request.clone()).await.unwrap_err();
    let again = clob.place(request).await.unwrap_err();

    assert_eq!(err.class, Class::Decode, "{err}");
    assert_eq!(again.class, Class::InvalidRequest, "{again}");
    assert!(again.to_string().contains("already in use"), "{again}");
    // The second place sent nothing.
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

#[tokio::test]
async fn cancelling_one_order_frees_the_client_order_id_the_ledger_held() {
    let mut server = Server::new_async().await;
    let post = post_ok(&mut server, LIVE, 2).await;
    let delete = delete_answering(
        &mut server,
        "/order",
        json!({"orderID": "0xorder1"}),
        r#"{"canceled":["0xorder1"],"notCanceled":{}}"#,
    )
    .await;
    let clob = client(&server);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    clob.place(request.clone()).await.unwrap();
    let refused = clob.place(request.clone()).await.unwrap_err();
    clob.cancel(&OrderId::new("0xorder1")).await.unwrap();
    let again = clob.place(request).await.unwrap();

    assert!(refused.to_string().contains("already in use"), "{refused}");
    assert_eq!(again.client_order_id, Some(ClientOrderId::new("mine-1")));
    post.assert_async().await;
    delete.assert_async().await;
}

#[tokio::test]
async fn a_price_or_size_that_needs_no_tick_is_refused_before_the_tick_is_looked_up() {
    let mut server = Server::new_async().await;
    let tick = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let untouched = nothing_sent(&mut server).await;
    let clob = client(&server);
    let mut options = ClobOrderOptions::default();
    options.neg_risk = Some(false);

    let price = clob
        .place(with_options(buy("1.5", "10"), options))
        .await
        .unwrap_err();
    let size = clob
        .place(with_options(buy("0.52", "5.001"), options))
        .await
        .unwrap_err();

    assert_eq!(price.class, Class::InvalidRequest, "{price}");
    assert!(price.to_string().contains("strictly between"), "{price}");
    assert_eq!(size.class, Class::InvalidRequest, "{size}");
    assert!(size.to_string().contains("decimal places"), "{size}");
    tick.assert_async().await;
    for mock in untouched {
        mock.assert_async().await;
    }
}

#[tokio::test]
async fn a_tick_size_the_clob_answers_that_is_unknown_is_a_decode_error() {
    let mut server = Server::new_async().await;
    let tick = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.05"}"#)
        .expect(1)
        .create_async()
        .await;
    let post = server.mock("POST", "/order").expect(0).create_async().await;
    let mut options = ClobOrderOptions::default();
    options.neg_risk = Some(false);

    let err = client(&server)
        .place(with_options(buy("0.55", "10"), options))
        .await
        .unwrap_err();

    assert_eq!(err.class, Class::Decode, "{err}");
    tick.assert_async().await;
    post.assert_async().await;
}

/// One event of the log capture: its level, target, and message and fields as
/// `name=value` text.
type Logged = (tracing::Level, String, String);

/// Records every event, for the tests that read the client's WARNs.
#[derive(Clone, Default)]
struct Captured(Arc<std::sync::Mutex<Vec<Logged>>>);

struct Fields(String);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        if field.name() == "message" {
            self.0.push_str(&format!("{value:?}"));
        } else {
            self.0.push_str(&format!("{}={value:?}", field.name()));
        }
    }
}

impl tracing::Subscriber for Captured {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        let metadata = event.metadata();
        self.0
            .lock()
            .unwrap()
            .push((*metadata.level(), metadata.target().to_owned(), fields.0));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

impl Captured {
    /// The WARNs whose text contains `needle`.
    fn warnings(&self, needle: &str) -> Vec<Logged> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(level, _, text)| *level == tracing::Level::WARN && text.contains(needle))
            .cloned()
            .collect()
    }
}

/// Starts capturing on this thread. Hold both returned values for the whole
/// test: with one dispatcher registered, tracing-core works out the interest
/// of a callsite another test's thread hits first from that thread's default,
/// which has none, and caches it as never for every thread; with a second one
/// registered it asks each registered dispatcher instead. The tests here run
/// on a current-thread runtime, so the scoped subscriber sees their events.
fn capture() -> (
    Captured,
    tracing::subscriber::DefaultGuard,
    tracing::Dispatch,
) {
    let captured = Captured::default();
    let scope = tracing::subscriber::set_default(captured.clone());
    let second = tracing::Dispatch::new(Captured::default());
    (captured, scope, second)
}

#[tokio::test]
async fn a_failed_read_back_of_a_matched_order_is_logged_as_a_warning() {
    let (logs, _scope, _second) = capture();
    let mut server = Server::new_async().await;
    let _lookups = lookups(&mut server).await;
    let _post = post_ok(&mut server, MATCHED, 1).await;
    let _read = server
        .mock("GET", "/data/order/0xorder1")
        .with_status(500)
        .with_body("upstream down")
        .expect(1)
        .create_async()
        .await;

    let order = client(&server).place(buy("0.52", "10")).await.unwrap();

    assert_eq!(order.status, OrderStatus::Open);
    let warned = logs.warnings("could not read back a matched order");
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(
        warned[0].1.starts_with("polyoxide_polymarket"),
        "{:?}",
        warned[0]
    );
    assert!(warned[0].2.contains("order=0xorder1"), "{:?}", warned[0]);
}

#[tokio::test]
async fn an_order_id_the_clob_names_differently_is_rekeyed_and_logged_as_a_warning() {
    let (logs, _scope, _second) = capture();
    let mut server = Server::new_async().await;
    let _post = post_ok(&mut server, LIVE, 1).await;
    let clob = client(&server);
    let mut request = with_options(buy("0.52", "10"), declared(TickSize::Hundredth));
    request.client_order_id = Some(ClientOrderId::new("mine-1"));

    let order = clob.place(request.clone()).await.unwrap();
    let refused = clob.place(request).await.unwrap_err();

    // The venue's `0xorder1` is not the order's computed hash.
    assert_eq!(order.id.as_str(), "0xorder1");
    assert!(refused.to_string().contains("`0xorder1`"), "{refused}");
    let warned = logs.warnings("the CLOB named an order other than the one computed");
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(
        warned[0].1.starts_with("polyoxide_polymarket"),
        "{:?}",
        warned[0]
    );
    assert!(warned[0].2.contains("venue=0xorder1"), "{:?}", warned[0]);
    assert!(warned[0].2.contains("computed=0x"), "{:?}", warned[0]);
}
