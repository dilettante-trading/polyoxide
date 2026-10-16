//! The example's `best_bid_ask` and `classify`, written once, run unchanged
//! against mock servers for the clob, perps and usdm modules: CAP-5's and
//! CAP-4's success criteria.

#[allow(dead_code)]
#[path = "../examples/venue_neutral.rs"]
mod example;

use std::sync::Arc;
use std::time::Duration;

use example::{best_bid_ask, classify, classify_classified, Handling};
use mockito::{Matcher, Server, ServerGuard};
use polyoxide::binance::usdm::{
    self,
    types::{DepthLimit, Symbol},
    Usdm,
};
use polyoxide::polymarket::clob::{Clob, ClobBuilder};
use polyoxide::polymarket::perps::{types::InstrumentId, Perps};
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{DynMarketData, Level, MarketKey};
use polyoxide_core::RetryConfig;
use rust_decimal::Decimal;

const TOKEN: &str = "54533043819946592547517511176940999955633860128497669742211153063842200957669";

/// The three modules the suite runs against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Usdm,
    Perps,
    Clob,
}

/// A module's client, its key, and the route and body its quote reads.
struct Module {
    kind: Kind,
    path: &'static str,
    query: String,
    body: &'static str,
}

fn modules() -> [Module; 3] {
    [
        Module {
            kind: Kind::Usdm,
            path: "/fapi/v1/depth",
            query: "symbol=BTCUSDT&limit=5".to_owned(),
            body: r#"{"lastUpdateId":1,"E":1791383684399,"T":1791383684397,"bids":[["83120.70","4.483"],["83120.60","0.002"]],"asks":[["83120.80","3.638"],["83120.90","0.001"]]}"#,
        },
        Module {
            kind: Kind::Perps,
            path: "/v1/info/bbo",
            query: "instrument_id=1".to_owned(),
            body: r#"[{"instrument_id":1,"bid_price":"7681.6","bid_quantity":"1.03204","ask_price":"7681.7","ask_quantity":"19.90711","timestamp":1790761918305}]"#,
        },
        Module {
            kind: Kind::Clob,
            path: "/book",
            query: format!("token_id={TOKEN}"),
            // Each side worst first, as the CLOB sends it.
            body: r#"{"market":"0x0f49","asset_id":"5453","timestamp":"1792161583225","hash":"h","bids":[{"price":"0.13","size":"10"},{"price":"0.15","size":"20"}],"asks":[{"price":"0.18","size":"10"},{"price":"0.16","size":"20"}],"min_order_size":"5","tick_size":"0.001","neg_risk":true,"last_trade_price":"0.15"}"#,
        },
    ]
}

/// No retries, so a refused request answers at once.
fn once() -> RetryConfig {
    RetryConfig {
        max_retries: 0,
        ..RetryConfig::default()
    }
}

fn usdm_client(server: &ServerGuard) -> Usdm {
    Usdm::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

fn perps_client(server: &ServerGuard) -> Perps {
    Perps::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

fn clob_client(server: &ServerGuard) -> Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

/// The module's client as a `DynMarketData`, and the key its quote reads.
fn venue(kind: Kind, server: &ServerGuard) -> (Arc<DynMarketData<'static>>, MarketKey) {
    match kind {
        Kind::Usdm => (
            DynMarketData::new_arc(usdm_client(server)),
            usdm::key(&Symbol::new("BTCUSDT").unwrap()),
        ),
        Kind::Perps => (
            DynMarketData::new_arc(perps_client(server)),
            keys::perps(InstrumentId(1)),
        ),
        Kind::Clob => (
            DynMarketData::new_arc(clob_client(server)),
            keys::clob(&TokenId::new(TOKEN).unwrap()),
        ),
    }
}

fn level(price: &str, size: &str) -> Level {
    Level::new(
        price.parse::<Decimal>().unwrap(),
        size.parse::<Decimal>().unwrap(),
    )
}

#[tokio::test]
async fn best_bid_ask_reads_each_module_unchanged() {
    let expected = [
        (
            Kind::Usdm,
            level("83120.70", "4.483"),
            level("83120.80", "3.638"),
        ),
        (
            Kind::Perps,
            level("7681.6", "1.03204"),
            level("7681.7", "19.90711"),
        ),
        (Kind::Clob, level("0.15", "20"), level("0.16", "20")),
    ];
    for (module, (kind, bid, ask)) in modules().into_iter().zip(expected) {
        assert_eq!(module.kind, kind);
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", module.path)
            .match_query(Matcher::Exact(module.query.clone()))
            .with_status(200)
            .with_body(module.body)
            .create_async()
            .await;
        let (venue, key) = venue(kind, &server);
        let quote = best_bid_ask(&venue, &key).await.unwrap();
        mock.assert_async().await;
        assert_eq!(quote, (Some(bid), Some(ask)), "{kind:?}");
    }
}

/// Fails the module's own quote route with `status`, through its native
/// client, and returns how the classifier reads that error.
async fn native(kind: Kind, server: &ServerGuard) -> Handling {
    match kind {
        Kind::Usdm => {
            let err = usdm_client(server)
                .market()
                .depth(&Symbol::new("BTCUSDT").unwrap())
                .limit(DepthLimit::Five)
                .send()
                .await
                .unwrap_err();
            classify(&err)
        }
        Kind::Perps => {
            let err = perps_client(server)
                .market()
                .bbo()
                .instrument_id(InstrumentId(1))
                .send()
                .await
                .unwrap_err();
            classify(&err)
        }
        Kind::Clob => {
            let err = clob_client(server)
                .markets()
                .order_book(TOKEN)
                .send()
                .await
                .unwrap_err();
            classify(&err)
        }
    }
}

#[tokio::test]
async fn one_classifier_reads_each_modules_errors_natively_and_through_the_traits() {
    let cases = [
        (
            429,
            Some("1"),
            Handling::Backoff(Some(Duration::from_secs(1))),
        ),
        (503, None, Handling::Retry),
        (401, None, Handling::Reauthenticate),
        (451, None, Handling::Restricted { alert: false }),
        (400, None, Handling::Rejected { alert: true }),
    ];
    for (status, retry_after, expected) in cases {
        for module in modules() {
            let mut server = Server::new_async().await;
            let mut mock = server
                .mock("GET", module.path)
                .match_query(Matcher::Any)
                .with_status(status)
                .with_body(r#"{"code":-1,"msg":"refused","error":"refused"}"#);
            if let Some(wait) = retry_after {
                mock = mock.with_header("retry-after", wait);
            }
            let _mock = mock.create_async().await;

            assert_eq!(
                native(module.kind, &server).await,
                expected,
                "{:?} {status}, native",
                module.kind
            );
            let (venue, key) = venue(module.kind, &server);
            let err = best_bid_ask(&venue, &key).await.unwrap_err();
            assert_eq!(
                classify_classified(&err),
                expected,
                "{:?} {status}, through the traits",
                module.kind
            );
        }
    }
}

#[test]
fn the_traits_are_on_polyoxide_venue() {
    fn market_data<T: polyoxide::venue::MarketData>() {}
    fn trades<T: polyoxide::venue::Trades>() {}
    fn candles<T: polyoxide::venue::Candles>() {}
    fn funding<T: polyoxide::venue::Funding>() {}
    fn instruments<T: polyoxide::venue::Instruments>() {}
    market_data::<Usdm>();
    market_data::<Perps>();
    market_data::<Clob>();
    trades::<Usdm>();
    candles::<Clob>();
    funding::<Perps>();
    instruments::<Usdm>();
}
