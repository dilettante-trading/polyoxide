//! Polymarket's composed CLOB throttle, driven through core's send loop
//! against a mock server.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mockito::{Matcher, Server, ServerGuard};
use polyoxide_core::polymarket::{
    clob_throttle, signer_cost, ClobThrottle, PolymarketRetryPolicy, CLOUDFLARE, SIGNER_ORDER,
};
use polyoxide_core::{
    ApiError, AttemptInfo, Charge, HttpClient, HttpClientBuilder, LayerCharge, Refused,
    RequestMeta, RequestParts, ResponseMeta, RetryConfig, Throttle, Tier, TradingRequest,
};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Method, StatusCode};

/// A client on `server` with Polymarket's policy, throttled by `throttle`.
fn client(server: &ServerGuard, throttle: impl Throttle + 'static, max_retries: u32) -> HttpClient {
    HttpClientBuilder::new(server.url())
        .with_throttle(throttle)
        .with_retry_policy(PolymarketRetryPolicy)
        .with_retry_config(RetryConfig {
            max_retries,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        })
        .build()
        .unwrap()
}

/// How long the IP layer and the signer layer each make a request wait.
async fn both_layers_wait(throttle: &ClobThrottle) -> (Duration, Duration) {
    let ip = async {
        let start = Instant::now();
        throttle.table().acquire("/book", Some(&Method::GET)).await;
        start.elapsed()
    };
    let signer = async {
        let start = Instant::now();
        throttle
            .signer()
            .acquire(TradingRequest::PostOrder)
            .await
            .unwrap();
        start.elapsed()
    };
    tokio::join!(ip, signer)
}

/// The CLOB throttle, recording what each attempt was charged.
#[derive(Clone)]
struct Charges {
    throttle: ClobThrottle,
    seen: Arc<Mutex<Vec<Vec<LayerCharge>>>>,
}

impl Throttle for Charges {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.throttle.acquire(meta).await
    }

    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, attempt: &AttemptInfo) {
        self.seen.lock().unwrap().push(charge.layers().to_vec());
        self.throttle.observe(charge, response, attempt);
    }

    fn hold(&self, delay: Duration) {
        self.throttle.hold(delay);
    }
}

#[tokio::test]
async fn charges_the_ip_layer_one_and_the_signer_layer_n() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/orders")
        .with_body("[]")
        .create_async()
        .await;
    let charges = Charges {
        throttle: clob_throttle(),
        seen: Arc::default(),
    };
    let http = client(&server, charges.clone(), 0);

    let costs = [signer_cost(TradingRequest::PostOrders { count: 60 })];
    let response = http
        .send(RequestParts::new(Method::POST, "/orders"), &costs, None)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    mock.assert_async().await;

    assert_eq!(
        *charges.seen.lock().unwrap(),
        [vec![
            LayerCharge {
                layer: CLOUDFLARE,
                units: 1,
                window: None
            },
            LayerCharge {
                layer: SIGNER_ORDER,
                units: 60,
                window: None
            },
        ]],
        "one request to Cloudflare, sixty orders to the signer"
    );
    // Standard's order bucket holds 60 and refills 40 a second: the batch
    // drained it, so a single order now waits about 25ms.
    let start = Instant::now();
    charges
        .throttle
        .signer()
        .acquire(TradingRequest::PostOrder)
        .await
        .unwrap();
    assert!(
        start.elapsed() >= Duration::from_millis(10),
        "the batch was not charged in full: the next order went after {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn an_exact_batch_above_capacity_is_refused_without_sending() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/orders")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let http = client(&server, clob_throttle(), 3);

    // Standard's order bucket holds at most 60.
    let costs = [signer_cost(TradingRequest::PostOrders { count: 100 })];
    let err = http
        .send(RequestParts::new(Method::POST, "/orders"), &costs, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ApiError::Refused(Refused {
                layer: SIGNER_ORDER,
                units: 100,
                capacity: 60
            })
        ),
        "{err:?}"
    );
    assert!(!err.is_retriable());
    mock.assert_async().await;
}

#[tokio::test]
async fn a_hold_on_the_throttle_stops_both_layers() {
    let throttle = clob_throttle();
    throttle.hold(Duration::from_millis(300));
    // Extended mid-wait, to 600ms from the start: both layers honour it.
    let extender = {
        let throttle = throttle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            throttle.hold(Duration::from_millis(500));
        })
    };

    let (ip, signer) = both_layers_wait(&throttle).await;
    extender.await.unwrap();
    for (layer, waited) in [("IP", ip), ("signer", signer)] {
        assert!(
            waited >= Duration::from_millis(550),
            "the {layer} layer went after {waited:?}, inside the extended hold"
        );
    }
}

#[tokio::test]
async fn the_hold_survives_a_tier_change() {
    let throttle = clob_throttle();
    throttle.hold(Duration::from_millis(400));

    let mut headers = HeaderMap::new();
    headers.insert("poly-ratelimit-tier", HeaderValue::from_static("gold"));
    throttle.observe(
        &Charge::none(),
        &ResponseMeta {
            status: StatusCode::OK,
            headers: &headers,
        },
        &AttemptInfo {
            attempt: 0,
            retries_left: 0,
        },
    );
    assert_eq!(throttle.signer().tier(), Tier::Gold);

    // 500 orders fit Gold's bucket of 600 and never Standard's 60, so this is
    // the new bucket; it still waits out the hold set before it existed.
    let start = Instant::now();
    throttle
        .signer()
        .acquire(TradingRequest::PostOrders { count: 500 })
        .await
        .expect("500 fits Gold's order burst");
    assert!(
        start.elapsed() >= Duration::from_millis(350),
        "the new tier's bucket went after {:?}, inside the hold",
        start.elapsed()
    );
}

#[tokio::test]
async fn a_tier_on_a_429_is_adopted() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/order")
        .with_status(429)
        .with_header("poly-ratelimit-tier", "gold")
        .create_async()
        .await;
    let throttle = clob_throttle();
    let http = client(&server, throttle.clone(), 0);

    let costs = [signer_cost(TradingRequest::PostOrder)];
    let response = http
        .send(RequestParts::new(Method::POST, "/order"), &costs, None)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    mock.assert_async().await;
    assert_eq!(
        throttle.signer().tier(),
        Tier::Gold,
        "the tier the venue reported on a 429 was dropped"
    );
}

#[tokio::test]
async fn a_429_holds_both_layers() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/book")
        .match_query(Matcher::Any)
        .with_status(429)
        .create_async()
        .await;
    let throttle = clob_throttle();
    // No retry left: the 429 still holds, for retry_delay(0), 300-500ms.
    let http = client(&server, throttle.clone(), 0);

    let response = http
        .send(RequestParts::new(Method::GET, "/book"), &[], None)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    mock.assert_async().await;

    let (ip, signer) = both_layers_wait(&throttle).await;
    for (layer, waited) in [("IP", ip), ("signer", signer)] {
        assert!(
            waited >= Duration::from_millis(250),
            "the {layer} layer went after {waited:?}, inside the 429's hold"
        );
    }
}
