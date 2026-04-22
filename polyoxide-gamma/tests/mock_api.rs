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
    let markets = gamma
        .markets()
        .list()
        .limit(5)
        .open(true)
        .send()
        .await
        .unwrap();

    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].id, "mkt-1");
    assert_eq!(markets[0].condition_id, "0xcond1");
    assert_eq!(markets[0].question, "Will it rain?");

    mock.assert_async().await;
}

#[tokio::test]
async fn list_markets_open_false_sends_closed_true() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "closed".into(),
            "true".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let markets = gamma.markets().list().open(false).send().await.unwrap();

    assert!(markets.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn market_deserializes_volume_renamed_fields() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets/vol-test")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "vol-test",
                "conditionId": "0xcond",
                "question": "Volume test?",
                "description": "Testing serde renames",
                "marketMakerAddress": "0xaddr",
                "volume24hr": 1500.5,
                "volume1wk": 10000.0,
                "volume1mo": 50000.0,
                "volume1yr": 200000.0,
                "denomationToken": "USDC",
                "volume24hrAmm": 100.0,
                "volume1wkClob": 9900.0
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let market = gamma.markets().get("vol-test").send().await.unwrap();

    assert_eq!(market.volume_24hr, Some(1500.5));
    assert_eq!(market.volume_1wk, Some(10000.0));
    assert_eq!(market.volume_1mo, Some(50000.0));
    assert_eq!(market.volume_1yr, Some(200000.0));
    assert_eq!(market.denomination_token, Some("USDC".into()));
    assert_eq!(market.volume_24hr_amm, Some(100.0));
    assert_eq!(market.volume_1wk_clob, Some(9900.0));

    mock.assert_async().await;
}

#[tokio::test]
async fn list_events_with_query_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/events")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".into(), "3".into()),
            Matcher::UrlEncoded("active".into(), "true".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "evt-1",
                "title": "Test Event",
                "slug": "test-event"
            }]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let events = gamma
        .events()
        .list()
        .limit(3)
        .active(true)
        .send()
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "evt-1");
    assert_eq!(events[0].title, Some("Test Event".into()));

    mock.assert_async().await;
}

#[tokio::test]
async fn get_event_by_id() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/events/evt-42")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "evt-42",
                "title": "Single Event",
                "slug": "single-event",
                "markets": [{
                    "id": "mkt-nested",
                    "conditionId": "0xcond",
                    "question": "Nested?",
                    "description": "Nested market",
                    "marketMakerAddress": "0xaddr"
                }]
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let event = gamma.events().get("evt-42").send().await.unwrap();

    assert_eq!(event.id, "evt-42");
    assert_eq!(event.title, Some("Single Event".into()));
    assert_eq!(event.markets.len(), 1);
    assert_eq!(event.markets[0].id, "mkt-nested");

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
    let err = gamma.markets().get("nonexistent").send().await.unwrap_err();

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
async fn get_many_fans_out_closed_true_and_false() {
    let mut server = Server::new_async().await;

    // Regex (not Matcher::UrlEncoded) because mockito 1.7's UrlEncoded matcher
    // cannot assert multiple values for the same key (`id=1&id=2`) — anchored
    // regex is the reliable way to pin a specific repeated-key query pair.
    let mock_closed = server
        .mock("GET", "/markets")
        .match_query(Matcher::AllOf(vec![
            Matcher::Regex(r"(^|&)id=1(&|$)".into()),
            Matcher::Regex(r"(^|&)id=2(&|$)".into()),
            Matcher::Regex(r"(^|&)closed=true(&|$)".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "1",
                "conditionId": "0xcond1",
                "question": "Closed market?",
                "description": "A closed market",
                "marketMakerAddress": "0xaddr1",
                "closed": true
            }]"#,
        )
        .expect(1)
        .create_async()
        .await;

    let mock_open = server
        .mock("GET", "/markets")
        .match_query(Matcher::AllOf(vec![
            Matcher::Regex(r"(^|&)id=1(&|$)".into()),
            Matcher::Regex(r"(^|&)id=2(&|$)".into()),
            Matcher::Regex(r"(^|&)closed=false(&|$)".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "2",
                "conditionId": "0xcond2",
                "question": "Open market?",
                "description": "An open market",
                "marketMakerAddress": "0xaddr2",
                "closed": false
            }]"#,
        )
        .expect(1)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let markets = gamma.markets().get_many([1i64, 2]).send().await.unwrap();

    assert_eq!(markets.len(), 2);
    let ids: Vec<&str> = markets.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"1"), "missing closed market in merged result");
    assert!(ids.contains(&"2"), "missing open market in merged result");
    assert!(
        markets.iter().any(|m| m.closed == Some(true)),
        "merged result should include the closed market"
    );

    mock_closed.assert_async().await;
    mock_open.assert_async().await;
}

#[tokio::test]
async fn get_many_empty_ids_short_circuits() {
    let mut server = Server::new_async().await;

    // Expect zero hits to /markets — an empty-id call must not touch the network.
    let mock = server
        .mock("GET", "/markets")
        .with_status(500)
        .expect(0)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let markets = gamma
        .markets()
        .get_many(Vec::<i64>::new())
        .send()
        .await
        .unwrap();

    assert!(markets.is_empty());
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
