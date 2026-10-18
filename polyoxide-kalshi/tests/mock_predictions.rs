//! The exchange-status route, the ping, the builder's refusal of a base URL with a path, and
//! Kalshi's three error bodies, on a mock server.
//!
//! The status body is the one the production host answered on 2026-10-18, recorded in
//! `docs/specs/kalshi/OBSERVED.md`.

use std::sync::Arc;

use mockito::{Matcher, Server, ServerGuard};
use polyoxide_core::{ApiError, RetryConfig};
use polyoxide_kalshi::predictions::{Predictions, PredictionsError};
use polyoxide_venue::{Class, Classify};

const STATUS: &str = "/trade-api/v2/exchange/status";

/// `GET /trade-api/v2/exchange/status`, byte for byte as the production host sent it.
const STATUS_BODY: &str = concat!(
    r#"{"exchange_active":true,"exchange_index_statuses":["#,
    r#"{"description":"Default","exchange_active":true,"exchange_index":0,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true},"#,
    r#"{"description":"Combos","exchange_active":true,"exchange_index":1,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true},"#,
    r#"{"description":"Crypto & Commodities","exchange_active":true,"exchange_index":2,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true},"#,
    r#"{"description":"Tennis, Baseball, Basketball","exchange_active":true,"exchange_index":3,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true}],"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true}"#,
);

const NOT_FOUND: &str = r#"{"error":{"code":"not_found","message":"not found"}}"#;
const UNAUTHENTICATED: &str = concat!(
    r#"{"error":{"code":"token_authentication_failure","#,
    r#""message":"token authentication failure"}}"#,
);
const BAD_PARAMETER: &str = concat!(
    r#"{"msg":"Invalid format for parameter limit: error binding string parameter: "#,
    r#"strconv.ParseInt: parsing \"abc\": invalid syntax"}"#,
);
const TOO_MANY: &str = r#"{"error":"too many requests"}"#;

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

/// A client that does not retry, so a `429` or a hold cannot reach the next case.
fn client_without_retries(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap()
}

/// `GET` the status route, answered with `status` and `body`, on a client that does not retry.
async fn fail(status: usize, body: &str) -> PredictionsError {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", STATUS)
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body)
        .create_async()
        .await;
    client_without_retries(&server)
        .exchange()
        .status()
        .send()
        .await
        .unwrap_err()
}

#[tokio::test]
async fn exchange_status_reads_the_body_the_host_sent() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_header("content-type", "application/json; charset=utf-8")
        .with_body(STATUS_BODY)
        .create_async()
        .await;

    let status = client(&server).exchange().status().send().await.unwrap();
    mock.assert_async().await;

    assert!(status.exchange_active);
    assert!(status.trading_active);
    assert_eq!(status.intra_exchange_transfers_active, Some(true));
    assert_eq!(status.exchange_estimated_resume_time, None);
    let rows: Vec<_> = status
        .exchange_index_statuses
        .iter()
        .map(|row| (row.exchange_index, row.description.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (0, "Default"),
            (1, "Combos"),
            (2, "Crypto & Commodities"),
            (3, "Tennis, Baseball, Basketball"),
        ]
    );
    assert!(status.exchange_index_statuses.iter().all(|row| {
        row.exchange_active && row.trading_active && row.intra_exchange_transfers_active
    }));
}

#[tokio::test]
async fn ping_answers_from_the_status_route_and_leaves_the_body_unread() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_body(STATUS_BODY)
        .expect(1)
        .create_async()
        .await;

    let pong = client(&server).health().ping().await.unwrap();
    assert_eq!(pong.response.status().as_u16(), 200);
    assert_eq!(pong.response.text().await.unwrap(), STATUS_BODY);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_ping_the_host_refuses_is_the_error_kalshis_decode_makes_of_it() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", STATUS)
        .with_status(404)
        .with_body(NOT_FOUND)
        .create_async()
        .await;

    let err = client_without_retries(&server)
        .health()
        .ping()
        .await
        .unwrap_err();
    assert!(matches!(err, PredictionsError::Venue(_)), "{err:?}");
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(Arc::from("not_found"))
        }
    );
}

#[test]
fn building_with_a_path_in_the_base_url_is_refused() {
    for url in [
        "http://127.0.0.1:1/proxy",
        "http://127.0.0.1:1/proxy/",
        "https://external-api.kalshi.com/trade-api/v2",
    ] {
        let err = Predictions::builder().base_url(url).build().unwrap_err();
        let expected = format!("the base URL must be a scheme and host with no path: {url}");
        assert!(
            matches!(&err, PredictionsError::Api(ApiError::Validation(message))
                if *message == expected),
            "{url}: {err:?}"
        );
        assert_eq!(err.class(), Class::InvalidRequest, "{url}");
    }
}

#[test]
fn a_bare_host_is_a_base_url_with_or_without_its_slash() {
    for url in ["http://127.0.0.1:1", "http://127.0.0.1:1/"] {
        assert!(
            Predictions::builder().base_url(url).build().is_ok(),
            "{url}"
        );
    }
}

#[tokio::test]
async fn each_error_shape_decodes_into_a_venue_error_with_its_class() {
    // (status, body, code, message, class)
    let rows = [
        (
            401,
            UNAUTHENTICATED,
            Some("token_authentication_failure"),
            "token authentication failure",
            Class::Unauthorized,
        ),
        (
            404,
            NOT_FOUND,
            Some("not_found"),
            "not found",
            Class::VenueRefusal {
                code: Some(Arc::from("not_found")),
            },
        ),
        (
            400,
            BAD_PARAMETER,
            None,
            "Invalid format for parameter limit: error binding string parameter: \
             strconv.ParseInt: parsing \"abc\": invalid syntax",
            Class::VenueRefusal { code: None },
        ),
        (
            429,
            TOO_MANY,
            None,
            "too many requests",
            Class::RateLimited { retry_after: None },
        ),
    ];
    for (status, body, code, message, class) in rows {
        let err = fail(status, body).await;
        let PredictionsError::Venue(venue) = &err else {
            panic!("{status}: not a Kalshi error: {err:?}");
        };
        assert_eq!(usize::from(venue.status), status);
        assert_eq!(venue.code.as_deref(), code, "{status}");
        assert_eq!(venue.message, message, "{status}");
        assert_eq!(venue.details, None, "{status}");
        assert_eq!(venue.retry_after, None, "{status}");
        assert_eq!(err.class(), class, "{status}");
    }
}

#[tokio::test]
async fn a_503_with_the_status_body_stays_cores_and_is_unavailable() {
    let err = fail(503, STATUS_BODY).await;
    assert!(
        matches!(&err, PredictionsError::Api(ApiError::Response(response))
            if response.status.as_u16() == 503 && response.body == STATUS_BODY),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::Unavailable { code: None });
    assert!(err.is_retriable());
}

#[tokio::test]
async fn a_503_is_not_retried() {
    // Core's default policy retries a 429 and fails every other status (AD-17).
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", STATUS)
        .with_status(503)
        .with_body(NOT_FOUND)
        .expect(1)
        .create_async()
        .await;

    let err = client(&server)
        .exchange()
        .status()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, PredictionsError::Venue(_)), "{err:?}");
    mock.assert_async().await;
}

#[tokio::test]
async fn a_429_is_retried_and_the_next_answer_is_returned() {
    // Core's default policy retries a 429 (D6), which Kalshi's client installs unchanged.
    let mut server = Server::new_async().await;
    let limited = server
        .mock("GET", STATUS)
        .with_status(429)
        .with_body(TOO_MANY)
        .expect(1)
        .create_async()
        .await;
    let answered = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_body(STATUS_BODY)
        .expect(1)
        .create_async()
        .await;

    // A short backoff, so the retry does not wait out the default half second.
    let status = Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 5,
        })
        .build()
        .unwrap()
        .exchange()
        .status()
        .send()
        .await
        .unwrap();
    assert!(status.exchange_active);
    limited.assert_async().await;
    answered.assert_async().await;
}
