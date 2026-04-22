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
async fn list_event_creators_hits_endpoint_with_params() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/events/creators")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".into(), "10".into()),
            Matcher::UrlEncoded("creator_handle".into(), "poly".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "1",
                "creatorName": "Polymarket",
                "creatorHandle": "poly"
            }]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let creators = gamma
        .events()
        .list_creators()
        .limit(10)
        .creator_handle("poly")
        .send()
        .await
        .unwrap();

    assert_eq!(creators.len(), 1);
    assert_eq!(creators[0].id, "1");
    assert_eq!(creators[0].creator_handle.as_deref(), Some("poly"));
    mock.assert_async().await;
}

#[tokio::test]
async fn get_event_creator_by_id() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/events/creators/42")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id": "42", "creatorName": "Poly"}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let creator = gamma.events().get_creator("42").send().await.unwrap();
    assert_eq!(creator.id, "42");
    assert_eq!(creator.creator_name.as_deref(), Some("Poly"));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_events_pagination_returns_wrapper() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/events/pagination")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".into(), "5".into()),
            Matcher::UrlEncoded("include_chat".into(), "true".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "data": [{"id": "e1"}],
                "pagination": {"hasMore": true, "totalResults": 42}
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let resp = gamma
        .events()
        .list_paginated()
        .limit(5)
        .include_chat(true)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "e1");
    let p = resp.pagination.unwrap();
    assert_eq!(p.has_more, Some(true));
    assert_eq!(p.total_results, Some(42));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_events_results_returns_vec() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/events/results")
        .match_query(Matcher::UrlEncoded("limit".into(), "3".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"id": "r1"}, {"id": "r2"}]"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let events = gamma.events().list_results().limit(3).send().await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, "r1");
    mock.assert_async().await;
}

#[tokio::test]
async fn list_events_keyset_returns_cursor() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/events/keyset")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".into(), "2".into()),
            Matcher::UrlEncoded("after_cursor".into(), "abc123".into()),
            Matcher::UrlEncoded("closed".into(), "false".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "events": [{"id": "k1", "title": "Keyset"}, {"id": "k2"}],
                "next_cursor": "next-token"
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let resp = gamma
        .events()
        .list_keyset()
        .limit(2)
        .after_cursor("abc123")
        .closed(false)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.events.len(), 2);
    assert_eq!(resp.events[0].id, "k1");
    assert_eq!(resp.events[0].title.as_deref(), Some("Keyset"));
    assert_eq!(resp.next_cursor.as_deref(), Some("next-token"));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_events_keyset_last_page_has_no_cursor() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/events/keyset")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"events": []}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let resp = gamma.events().list_keyset().send().await.unwrap();
    assert!(resp.events.is_empty());
    assert!(resp.next_cursor.is_none());
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

// ── /markets/{id}/description ──────────────────────────────────

#[tokio::test]
async fn get_market_description_returns_text() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets/12345/description")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"description": "Will X happen?"}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let desc = gamma
        .markets()
        .get_description("12345")
        .send()
        .await
        .unwrap();
    assert_eq!(desc.description.as_deref(), Some("Will X happen?"));
    mock.assert_async().await;
}

// ── /markets/information (POST) ────────────────────────────────

#[tokio::test]
async fn query_markets_by_information_posts_body() {
    use polyoxide_gamma::types::MarketsInformationBody;

    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/markets/information")
        .match_header("content-type", "application/json")
        .match_body(Matcher::JsonString(
            r#"{"id":[1,2],"closed":true}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "1",
                "conditionId": "0xcond",
                "description": "Desc",
                "question": "Q?",
                "marketMakerAddress": "0xmm"
            }]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let body = MarketsInformationBody {
        id: vec![1, 2],
        closed: Some(true),
        ..Default::default()
    };
    let markets = gamma.markets().query_by_information(&body).await.unwrap();
    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].id, "1");
    mock.assert_async().await;
}

// ── /markets/abridged (POST) ───────────────────────────────────

#[tokio::test]
async fn query_abridged_markets_posts_body() {
    use polyoxide_gamma::types::MarketsInformationBody;

    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/markets/abridged")
        .match_header("content-type", "application/json")
        .match_body(Matcher::JsonString(
            r#"{"slug":["mkt-a"],"includeTags":true}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": "7",
                "conditionId": "0xcond7",
                "description": "Desc",
                "question": "Q?",
                "marketMakerAddress": "0xmm"
            }]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let body = MarketsInformationBody {
        slug: vec!["mkt-a".into()],
        include_tags: Some(true),
        ..Default::default()
    };
    let markets = gamma.markets().query_abridged(&body).await.unwrap();
    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].id, "7");
    mock.assert_async().await;
}

// ── /markets/keyset ────────────────────────────────────────────

#[tokio::test]
async fn list_markets_keyset_returns_cursor() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets/keyset")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("limit".into(), "2".into()),
            Matcher::UrlEncoded("after_cursor".into(), "abc123".into()),
            Matcher::UrlEncoded("closed".into(), "false".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "markets": [
                    {
                        "id": "k1",
                        "conditionId": "0xcond1",
                        "description": "d1",
                        "question": "q1",
                        "marketMakerAddress": "0xmm1"
                    },
                    {
                        "id": "k2",
                        "conditionId": "0xcond2",
                        "description": "d2",
                        "question": "q2",
                        "marketMakerAddress": "0xmm2"
                    }
                ],
                "next_cursor": "next-token"
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let resp = gamma
        .markets()
        .list_keyset()
        .limit(2)
        .after_cursor("abc123")
        .closed(false)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.markets.len(), 2);
    assert_eq!(resp.markets[0].id, "k1");
    assert_eq!(resp.next_cursor.as_deref(), Some("next-token"));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_markets_keyset_last_page_has_no_cursor() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets/keyset")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"markets": []}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let resp = gamma.markets().list_keyset().send().await.unwrap();
    assert!(resp.markets.is_empty());
    assert!(resp.next_cursor.is_none());
    mock.assert_async().await;
}

// ── /series-summary/{id} and /series-summary/slug/{slug} ───────

#[tokio::test]
async fn get_series_summary_by_id() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/series-summary/s-1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "s-1",
                "title": "NFL",
                "slug": "nfl",
                "eventDates": ["2025-09-01"],
                "eventWeeks": [1, 2],
                "earliest_open_week": 1
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let summary = gamma.series().get_summary("s-1").send().await.unwrap();
    assert_eq!(summary.id, "s-1");
    assert_eq!(summary.title.as_deref(), Some("NFL"));
    assert_eq!(summary.event_weeks, vec![1, 2]);
    mock.assert_async().await;
}

#[tokio::test]
async fn get_series_summary_by_slug() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/series-summary/slug/nfl")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id": "s-1", "slug": "nfl"}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let summary = gamma
        .series()
        .get_summary_by_slug("nfl")
        .send()
        .await
        .unwrap();
    assert_eq!(summary.slug.as_deref(), Some("nfl"));
    mock.assert_async().await;
}

// ── /series/{id}/comments/count ────────────────────────────────

#[tokio::test]
async fn get_series_comment_count() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/series/s-1/comments/count")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"count": 42}"#)
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let count = gamma.series().comment_count("s-1").send().await.unwrap();
    assert_eq!(count.count, 42);
    mock.assert_async().await;
}

// ── /teams/{id} ────────────────────────────────────────────────

#[tokio::test]
async fn get_team_by_id() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/teams/100")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": 100,
                "name": "Team X",
                "league": "NFL",
                "abbreviation": "TX"
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let team = gamma.sports().get_team("100").send().await.unwrap();
    assert_eq!(team.id, 100);
    assert_eq!(team.name.as_deref(), Some("Team X"));
    mock.assert_async().await;
}

// ── /profiles/user_address/{address} ───────────────────────────

#[tokio::test]
async fn get_profile_by_address() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/profiles/user_address/0xdeadbeef")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "p-1",
                "name": "Alice",
                "proxyWallet": "0xdeadbeef",
                "walletActivated": true
            }"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let profile = gamma
        .user()
        .get_by_address("0xdeadbeef")
        .send()
        .await
        .unwrap();
    assert_eq!(profile.id, "p-1");
    assert_eq!(profile.name.as_deref(), Some("Alice"));
    assert_eq!(profile.proxy_wallet.as_deref(), Some("0xdeadbeef"));
    assert_eq!(profile.wallet_activated, Some(true));
    mock.assert_async().await;
}

#[tokio::test]
async fn get_related_detailed_returns_vec_of_tags() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/tags/42/related-tags/tags")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {
                    "id": "101",
                    "slug": "politics",
                    "label": "Politics",
                    "forceShow": true,
                    "publishedAt": "2024-01-01T00:00:00Z",
                    "isCarousel": false
                },
                {
                    "id": "102",
                    "slug": "elections",
                    "label": "Elections"
                }
            ]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let tags = gamma
        .tags()
        .get_related_detailed("42")
        .send()
        .await
        .unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].id, "101");
    assert_eq!(tags[0].slug, "politics");
    assert_eq!(tags[0].label, "Politics");
    assert_eq!(tags[0].force_show, Some(true));
    assert_eq!(tags[0].is_carousel, Some(false));
    assert_eq!(tags[1].id, "102");
    assert_eq!(tags[1].force_show, None);
    mock.assert_async().await;
}

#[tokio::test]
async fn get_related_detailed_by_slug_returns_vec_of_tags() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/tags/slug/politics/related-tags/tags")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {
                    "id": "55",
                    "slug": "us-election",
                    "label": "US Election"
                }
            ]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let tags = gamma
        .tags()
        .get_related_detailed_by_slug("politics")
        .send()
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, "55");
    assert_eq!(tags[0].slug, "us-election");
    assert_eq!(tags[0].label, "US Election");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_related_detailed_by_slug_url_encodes_slug() {
    let mut server = Server::new_async().await;

    // Slug with characters that require URL encoding
    let mock = server
        .mock("GET", "/tags/slug/us%2Felection/related-tags/tags")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let tags = gamma
        .tags()
        .get_related_detailed_by_slug("us/election")
        .send()
        .await
        .unwrap();
    assert!(tags.is_empty());
    mock.assert_async().await;
}
