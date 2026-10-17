//! `Trading` on the CLOB against mock servers: the HTTP routes run on
//! mockito and the user socket on the kit's scripted server. The account is
//! Hardhat account 0, an EOA whose maker address is its own.

use std::borrow::Cow;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::clob::venue::{BalanceExt, PositionExt};
use polyoxide_polymarket::clob::venue::{ClobOrderOptions, OrderExt};
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
