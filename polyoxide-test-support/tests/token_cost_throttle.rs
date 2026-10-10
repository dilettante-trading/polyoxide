//! A token-cost throttle built outside core, from core's public primitives
//! alone (CAP-2).
//!
//! The shape is a per-account venue that publishes two token buckets, one
//! for reads and one for writes, each with a capacity and a refill rate, and
//! charges each request an integer number of tokens from one of them. Nothing
//! here needed an edit to `polyoxide-core`: the throttle is two
//! [`CapacityBucket`]s over one [`Hold`], and the request builder hands
//! [`HttpClient::send`] the [`Cost`] of each request.

use std::time::{Duration, Instant};

use mockito::{Matcher, Server};
use polyoxide_core::reqwest::{Method, StatusCode};
use polyoxide_core::{
    ApiError, AttemptInfo, CapacityBucket, Charge, Cost, Hold, HttpClient, HttpClientBuilder,
    LayerCharge, LayerId, Refused, RequestMeta, RequestParts, ResponseMeta, Throttle,
};

const READ: LayerId = LayerId("read");
const WRITE: LayerId = LayerId("write");

/// Separate read and write buckets over one hold.
#[derive(Clone)]
struct TokenCostThrottle {
    read: CapacityBucket,
    write: CapacityBucket,
    hold: Hold,
}

impl TokenCostThrottle {
    /// Both buckets confirmed, each `(capacity, refill per second)`.
    fn new(read: (u32, u32), write: (u32, u32)) -> Self {
        let hold = Hold::unbounded();
        Self {
            read: CapacityBucket::new(READ, read.0, read.1, hold.clone()),
            write: CapacityBucket::new(WRITE, write.0, write.1, hold.clone()),
            hold,
        }
    }

    /// The write bucket sized from a guess, before the venue has said what
    /// this account's capacity is.
    fn provisional_write(capacity: u32, refill_per_sec: u32) -> Self {
        let hold = Hold::unbounded();
        Self {
            read: CapacityBucket::new(READ, 100, 100, hold.clone()),
            write: CapacityBucket::provisional(WRITE, capacity, refill_per_sec, hold.clone()),
            hold,
        }
    }

    fn bucket(&self, layer: LayerId) -> Option<&CapacityBucket> {
        if layer == READ {
            Some(&self.read)
        } else if layer == WRITE {
            Some(&self.write)
        } else {
            None
        }
    }

    /// Charge one request costing `units` of `layer`, as the send loop would.
    async fn charge(&self, layer: LayerId, units: u32) -> Result<Charge, Refused> {
        let costs = [cost(layer, units)];
        self.acquire(&RequestMeta {
            method: &Method::POST,
            path: "/orders",
            query: &[],
            costs: &costs,
        })
        .await
    }
}

impl Throttle for TokenCostThrottle {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.hold.wait().await;
        let mut charge = Charge::none();
        for cost in meta.costs {
            let Some(bucket) = self.bucket(cost.layer) else {
                continue;
            };
            bucket.acquire(cost.units, cost.exact).await?;
            charge = charge.with(LayerCharge {
                layer: cost.layer,
                units: cost.units,
                window: None,
            });
        }
        self.hold.wait().await;
        Ok(charge)
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, delay: Duration) {
        self.hold.extend(delay);
    }
}

fn cost(layer: LayerId, units: u32) -> Cost {
    Cost {
        layer,
        units,
        exact: true,
    }
}

/// How long charging `units` of `layer` took.
async fn timed(throttle: &TokenCostThrottle, layer: LayerId, units: u32) -> Duration {
    let start = Instant::now();
    throttle.charge(layer, units).await.unwrap();
    start.elapsed()
}

fn client(server: &mockito::ServerGuard, throttle: &TokenCostThrottle) -> HttpClient {
    HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .build()
        .unwrap()
}

#[tokio::test]
async fn the_read_and_write_layers_are_independent() {
    let throttle = TokenCostThrottle::new((10, 10), (10, 10));
    timed(&throttle, WRITE, 10).await;

    assert!(
        timed(&throttle, READ, 1).await < Duration::from_millis(25),
        "a read waited on the drained write bucket"
    );
    assert!(
        timed(&throttle, WRITE, 1).await >= Duration::from_millis(80),
        "the write bucket was not drained"
    );
}

#[tokio::test]
async fn a_ten_unit_write_drains_ten() {
    // 15 tokens: after a 10-unit write, 5 go at once and a sixth waits for
    // the refill, so the write took exactly 10.
    let throttle = TokenCostThrottle::new((100, 100), (15, 20));
    let charge = throttle.charge(WRITE, 10).await.unwrap();
    assert_eq!(
        charge.layers(),
        [LayerCharge {
            layer: WRITE,
            units: 10,
            window: None
        }]
    );
    assert!(timed(&throttle, WRITE, 5).await < Duration::from_millis(25));
    assert!(
        timed(&throttle, WRITE, 1).await >= Duration::from_millis(40),
        "the 10-unit write took fewer than 10 tokens"
    );
}

#[tokio::test]
async fn an_above_capacity_cost_is_refused_once_confirmed_and_waits_while_provisional() {
    // Confirmed: refused at once, through the loop, with nothing sent.
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/orders")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let confirmed = TokenCostThrottle::new((100, 100), (10, 10));
    let err = client(&server, &confirmed)
        .send(
            RequestParts::new(Method::POST, "/orders"),
            &[cost(WRITE, 11)],
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ApiError::Refused(Refused {
                layer: WRITE,
                units: 11,
                capacity: 10
            })
        ),
        "{err:?}"
    );
    mock.assert_async().await;

    // Provisional: it waits, then is refused once the size is confirmed...
    let guessed = TokenCostThrottle::provisional_write(10, 10);
    let waiting = tokio::spawn({
        let throttle = guessed.clone();
        async move { throttle.charge(WRITE, 11).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !waiting.is_finished(),
        "a provisional bucket decided too soon"
    );
    guessed.write.confirm();
    assert_eq!(
        waiting.await.unwrap(),
        Err(Refused {
            layer: WRITE,
            units: 11,
            capacity: 10
        })
    );

    // ...or charged once a resize makes room for it.
    let guessed = TokenCostThrottle::provisional_write(10, 10);
    let waiting = tokio::spawn({
        let throttle = guessed.clone();
        async move { throttle.charge(WRITE, 11).await.map(|_| ()) }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!waiting.is_finished());
    guessed.write.resize(20, 10);
    assert_eq!(waiting.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn resize_keeps_the_tokens_and_the_hold() {
    let throttle = TokenCostThrottle::new((100, 100), (10, 10));
    timed(&throttle, WRITE, 6).await;

    // Growing keeps the 4 tokens left rather than filling to 100.
    throttle.write.resize(100, 10);
    assert!(timed(&throttle, WRITE, 4).await < Duration::from_millis(25));
    assert!(
        timed(&throttle, WRITE, 1).await >= Duration::from_millis(50),
        "the resize filled the bucket"
    );

    // A hold set before a resize still holds after it.
    throttle.hold(Duration::from_millis(200));
    throttle.write.resize(50, 10);
    assert!(timed(&throttle, WRITE, 1).await >= Duration::from_millis(150));
}

#[tokio::test]
async fn a_hold_stops_both_layers() {
    let mut server = Server::new_async().await;
    let read = server
        .mock("GET", "/markets")
        .match_query(Matcher::Any)
        .create_async()
        .await;
    let write = server
        .mock("POST", "/orders")
        .match_query(Matcher::Any)
        .create_async()
        .await;
    let throttle = TokenCostThrottle::new((10, 10), (10, 10));
    let http = client(&server, &throttle);
    throttle.hold(Duration::from_millis(200));

    let send = |method: Method, path: &'static str, layer: LayerId| {
        let http = http.clone();
        async move {
            let start = Instant::now();
            let response = http
                .send(RequestParts::new(method, path), &[cost(layer, 1)], None)
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            start.elapsed()
        }
    };
    let (read_waited, write_waited) = tokio::join!(
        send(Method::GET, "/markets", READ),
        send(Method::POST, "/orders", WRITE),
    );
    read.assert_async().await;
    write.assert_async().await;
    for (layer, waited) in [("read", read_waited), ("write", write_waited)] {
        assert!(
            waited >= Duration::from_millis(150),
            "the {layer} layer went after {waited:?}, inside the hold"
        );
    }
}
