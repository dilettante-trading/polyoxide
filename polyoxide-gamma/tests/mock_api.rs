use mockito::{Matcher, Server};
use polyoxide_gamma::{Gamma, GammaError};

fn test_gamma(server: &mockito::ServerGuard) -> Gamma {
    Gamma::builder().base_url(server.url()).build().unwrap()
}

#[tokio::test]
async fn list_markets_with_query_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".into(), "5".into()),
            Matcher::UrlEncoded("closed".into(), "false".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "mkt-1",
                "conditionId": "0xcond1",
                "question": "Will it rain?",
                "description": "Market about weather",
                "marketMakerAddress": "0xaddr1"
            }]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let markets = gamma.markets().list().limit(5).open(true).send().await.unwrap();

    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].id, "mkt-1");
    assert_eq!(markets[0].condition_id, "0xcond1");
    assert_eq!(markets[0].question, "Will it rain?");

    mock.assert_async().await;
}

#[tokio::test]
async fn get_market_by_id() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets/abc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "abc",
                "conditionId": "0xcondabc",
                "question": "Single market?",
                "description": "A single market",
                "marketMakerAddress": "0xmaker"
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let market = gamma.markets().get("abc").send().await.unwrap();

    assert_eq!(market.id, "abc");
    assert_eq!(market.condition_id, "0xcondabc");

    mock.assert_async().await;
}

#[tokio::test]
async fn error_404_returns_api_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets/nonexistent")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "not found"}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let err = gamma
        .markets()
        .get("nonexistent")
        .send()
        .await
        .unwrap_err();

    match err {
        GammaError::Api(polyoxide_core::ApiError::Api { status, message }) => {
            assert_eq!(status, 404);
            assert_eq!(message, "not found");
        }
        other => panic!("Expected ApiError::Api(404), got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn error_401_returns_authentication_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets/secret")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "unauthorized"}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let err = gamma.markets().get("secret").send().await.unwrap_err();

    match err {
        GammaError::Api(polyoxide_core::ApiError::Authentication(msg)) => {
            assert_eq!(msg, "unauthorized");
        }
        other => panic!("Expected Authentication error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn error_400_returns_validation_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "invalid limit parameter"}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let err = gamma.markets().list().send().await.unwrap_err();

    match err {
        GammaError::Api(polyoxide_core::ApiError::Validation(msg)) => {
            assert_eq!(msg, "invalid limit parameter");
        }
        other => panic!("Expected Validation error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn malformed_json_returns_serialization_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets/bad")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"not_valid_market_json": true"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let err = gamma.markets().get("bad").send().await.unwrap_err();

    match err {
        GammaError::Api(polyoxide_core::ApiError::Serialization(_)) => {}
        other => panic!("Expected Serialization error, got: {:?}", other),
    }

    mock.assert_async().await;
}
