//! The class each status reaches a caller with, read through `Classify` alone.
//!
//! One request per status, each on a fresh client with no retries, so a `429`
//! hold cannot reach the next case. These pin the classes the error reshape
//! (Story 3.11) keeps, so their assertions never change.

use mockito::{Matcher, Server};
use polyoxide_core::RetryConfig;
use polyoxide_perps::{Perps, PerpsError};
use polyoxide_venue::{Class, Classify};

/// `GET /v1/info/time`, answered with `status` and `body`, on a client that
/// does not retry.
async fn fail(status: usize, body: &str) -> PerpsError {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body)
        .create_async()
        .await;
    let perps = Perps::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap();
    perps.health().time().send().await.unwrap_err()
}

#[tokio::test]
async fn every_status_reaches_the_caller_with_its_class() {
    let refusal = Class::VenueRefusal { code: None };
    let unavailable = Class::Unavailable { code: None };
    // (status, class, is_fault), each with a body not in the venue's shape.
    let rows = [
        (400, refusal.clone(), true),
        (401, Class::Unauthorized, true),
        (403, Class::Unauthorized, true),
        (404, refusal, true),
        (408, unavailable.clone(), true),
        (418, Class::Restricted, true),
        (425, unavailable.clone(), true),
        (429, Class::RateLimited { retry_after: None }, true),
        // A region block is the venue answering as designed.
        (451, Class::Restricted, false),
        (500, unavailable.clone(), true),
        (503, unavailable, true),
    ];
    for (status, class, fault) in rows {
        let err = fail(status, "<html>x</html>").await;
        assert_eq!(err.class(), class, "{status}: {err:?}");
        assert_eq!(err.is_fault(), fault, "{status}: {err:?}");
    }
}

#[tokio::test]
async fn the_venue_body_carries_its_code_into_the_class() {
    let code = |c: &str| Some(std::sync::Arc::from(c));
    // (status, class, is_fault), each with the venue's `{status, error}` body.
    let rows = [
        (
            404,
            Class::VenueRefusal {
                code: code("not_found"),
            },
            true,
        ),
        (429, Class::RateLimited { retry_after: None }, true),
        (
            503,
            Class::Unavailable {
                code: code("not_found"),
            },
            true,
        ),
        (451, Class::Restricted, false),
    ];
    for (status, class, fault) in rows {
        let err = fail(status, r#"{"status":"err","error":"not_found"}"#).await;
        assert_eq!(err.class(), class, "{status}: {err:?}");
        assert_eq!(err.is_fault(), fault, "{status}: {err:?}");
    }
}

#[tokio::test]
async fn a_2xx_body_that_does_not_decode_is_decode() {
    let err = fail(200, "not json").await;
    assert_eq!(err.class(), Class::Decode, "{err:?}");
    assert!(err.is_fault(), "{err:?}");
}
