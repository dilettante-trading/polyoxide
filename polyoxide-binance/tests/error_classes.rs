//! The class each status reaches a caller with, read through `Classify` alone.
//!
//! One request per status, each on a fresh client with no retries, so a `429`
//! or `418` hold cannot reach the next case. These pin the classes the error
//! reshape (Stories 3.11 and 3.12) keeps, so their assertions never change.

use mockito::{Matcher, Server};
use polyoxide_binance::{BinanceError, Usdm};
use polyoxide_core::RetryConfig;
use polyoxide_venue::{Class, Classify};

/// `GET /fapi/v1/time`, answered with `status` and `body`, on a client that
/// does not retry.
async fn fail(status: usize, body: &str) -> BinanceError {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/fapi/v1/time")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body)
        .create_async()
        .await;
    let usdm = Usdm::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap();
    usdm.health().time().send().await.unwrap_err()
}

#[tokio::test]
async fn every_status_reaches_the_caller_with_its_class() {
    let refusal = Class::VenueRefusal { code: None };
    let unavailable = Class::Unavailable { code: None };
    // (status, class, is_fault), each with a body not in Binance's shape. The
    // firewall's 403 is restricted (D14), and only a 451 is not a fault.
    let rows = [
        (400, refusal.clone(), true),
        (401, Class::Unauthorized, true),
        (403, Class::Restricted, true),
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
        let err = fail(status, "<html>x</html>").await;
        assert_eq!(err.class(), class, "{status}: {err:?}");
        assert_eq!(err.is_fault(), fault, "{status}: {err:?}");
    }
}

#[tokio::test]
async fn binance_s_body_carries_its_code_into_the_class() {
    let err = fail(400, r#"{"code":-1121,"msg":"Invalid symbol."}"#).await;
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(std::sync::Arc::from("-1121"))
        },
        "{err:?}"
    );
    assert!(err.is_fault(), "{err:?}");
}

#[tokio::test]
async fn a_2xx_body_that_does_not_decode_is_decode() {
    let err = fail(200, "not json").await;
    assert_eq!(err.class(), Class::Decode, "{err:?}");
    assert!(err.is_fault(), "{err:?}");
}
