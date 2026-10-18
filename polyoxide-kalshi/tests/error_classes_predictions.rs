//! The class each status reaches a caller with, read through `Classify` alone.
//!
//! One request per status, each on a fresh client with no retries, so a `429` hold cannot reach
//! the next case. A status decides the class before the body does (AD-15), so every row is run
//! with a body Kalshi does not use as well as with one it does.

use std::sync::Arc;
use std::time::Duration;

use mockito::{Matcher, Server};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::key::{InvalidTicker, Ticker};
use polyoxide_kalshi::predictions::{Predictions, PredictionsError};
use polyoxide_venue::{Class, Classify};

const STATUS: &str = "/trade-api/v2/exchange/status";

/// `GET` the status route, answered with `status`, `body` and, when given, a `Retry-After`, on a
/// client that does not retry.
async fn fail(status: usize, body: &str, retry_after: Option<&str>) -> PredictionsError {
    let mut server = Server::new_async().await;
    let mut mock = server
        .mock("GET", STATUS)
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    if let Some(wait) = retry_after {
        mock = mock.with_header("retry-after", wait);
    }
    let _mock = mock.create_async().await;
    let kalshi = Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap();
    kalshi.exchange().status().send().await.unwrap_err()
}

#[tokio::test]
async fn every_status_reaches_the_caller_with_its_class() {
    let refusal = Class::VenueRefusal { code: None };
    let unavailable = Class::Unavailable { code: None };
    // (status, class, is_fault). Only a 451 is the venue answering as designed.
    let rows = [
        (400, refusal.clone(), true),
        (401, Class::Unauthorized, true),
        (403, Class::Unauthorized, true),
        (404, refusal, true),
        (408, unavailable.clone(), true),
        (418, Class::Restricted, true),
        (425, unavailable.clone(), true),
        (429, Class::RateLimited { retry_after: None }, true),
        (451, Class::Restricted, false),
        (500, unavailable.clone(), true),
        (503, unavailable, true),
    ];
    for (status, class, fault) in rows {
        // A body that is not Kalshi's: core's error, classed by its status.
        let plain = fail(status, "<html>x</html>", None).await;
        assert!(
            matches!(plain, PredictionsError::Api(_)),
            "{status}: {plain:?}"
        );
        assert_eq!(plain.class(), class, "{status}: {plain:?}");
        assert_eq!(plain.is_fault(), fault, "{status}: {plain:?}");

        // One that is: Kalshi's error, classed by the same status.
        let venue = fail(status, r#"{"error":"x"}"#, None).await;
        assert!(
            matches!(venue, PredictionsError::Venue(_)),
            "{status}: {venue:?}"
        );
        assert_eq!(venue.class(), class, "{status}: {venue:?}");
        assert_eq!(venue.is_fault(), fault, "{status}: {venue:?}");
        assert_eq!(venue.is_retriable(), class.is_retriable(), "{status}");
    }
}

#[tokio::test]
async fn kalshis_code_reaches_the_class_where_the_class_has_room_for_one() {
    let err = fail(
        404,
        r#"{"error":{"code":"not_found","message":"not found"}}"#,
        None,
    )
    .await;
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(Arc::from("not_found"))
        },
        "{err:?}"
    );
    assert!(err.is_fault(), "{err:?}");
    assert!(!err.is_retriable(), "{err:?}");

    // `Unauthorized` has nowhere to put a code.
    let denied = fail(
        401,
        r#"{"error":{"code":"token_authentication_failure","message":"x"}}"#,
        None,
    )
    .await;
    assert_eq!(denied.class(), Class::Unauthorized, "{denied:?}");
}

#[tokio::test]
async fn a_retry_after_is_the_errors_wait_whichever_shape_the_body_has() {
    let wait = Some(Duration::from_secs(7));
    for body in [r#"{"error":"too many requests"}"#, "<html>x</html>"] {
        let limited = fail(429, body, Some("7")).await;
        assert_eq!(limited.retry_after(), wait, "{body}");
        assert_eq!(
            limited.class(),
            Class::RateLimited { retry_after: wait },
            "{body}"
        );
        assert!(limited.is_retriable(), "{body}");
    }

    // A 503 carries the wait too, though its class has no room for it.
    let down = fail(503, r#"{"error":"x"}"#, Some("2")).await;
    assert_eq!(down.retry_after(), Some(Duration::from_secs(2)));
    assert_eq!(down.class(), Class::Unavailable { code: None });

    // Kalshi sends none with a 429, and a zero is no wait.
    assert_eq!(fail(429, "x", None).await.retry_after(), None);
    assert_eq!(fail(429, "x", Some("0")).await.retry_after(), None);
}

#[tokio::test]
async fn a_2xx_body_that_does_not_decode_is_decode() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_body("not json")
        .create_async()
        .await;
    let kalshi = Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap();
    let err = kalshi.exchange().status().send().await.unwrap_err();
    assert_eq!(err.class(), Class::Decode, "{err:?}");
    assert!(err.is_fault(), "{err:?}");
}

#[test]
fn a_refusal_made_before_sending_is_an_invalid_request() {
    let path = Predictions::builder()
        .base_url("http://127.0.0.1:1/proxy")
        .build()
        .unwrap_err();
    assert_eq!(path.class(), Class::InvalidRequest, "{path:?}");
    assert!(path.is_fault(), "{path:?}");
    assert_eq!(path.retry_after(), None);

    let url = Predictions::builder()
        .base_url("not a url")
        .build()
        .unwrap_err();
    assert_eq!(url.class(), Class::InvalidRequest, "{url:?}");

    let ticker: InvalidTicker = Ticker::new("BTC/USD").unwrap_err();
    assert_eq!(ticker.class(), Class::InvalidRequest);
    assert!(ticker.is_fault());
    assert!(!ticker.is_retriable());
}
