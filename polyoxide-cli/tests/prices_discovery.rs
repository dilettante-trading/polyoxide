use mockito::{Matcher, Server};
use polyoxide_cli::commands::polymarket::clob::prices::select::{
    discover_targets, DiscoverFilters,
};

#[tokio::test]
async fn discover_extracts_token_ids_from_markets() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("closed".into(), "false".into()),
            Matcher::UrlEncoded("volume_num_min".into(), "1000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"id":"1","conditionId":"c1","question":"q1","marketMakerAddress":"0x1","clobTokenIds":"[\"111\",\"222\"]"},
                {"id":"2","conditionId":"c2","question":"q2","marketMakerAddress":"0x2","clobTokenIds":"[\"333\"]"},
                {"id":"3","conditionId":"c3","question":"q3","marketMakerAddress":"0x3"}
            ]"#,
        )
        .create_async()
        .await;

    let gamma = polyoxide_polymarket::gamma::Gamma::builder()
        .base_url(server.url())
        .build()
        .unwrap();

    let filters = DiscoverFilters {
        open: Some(true),
        min_volume: Some(1000.0),
        ..Default::default()
    };
    let ids = discover_targets(&gamma, &filters).await.unwrap();

    // Market 3 has no clobTokenIds and is skipped.
    assert_eq!(ids, ["111", "222", "333"]);
    mock.assert_async().await;
}

/// A Protocol V2 market's `clobTokenIds` has no book: discovery takes each
/// market's ids from the field its `version` names. Every expected id is read
/// off the JSON the server serves, so the test cannot agree with the code by
/// restating a constant.
#[tokio::test]
async fn discover_picks_the_id_field_by_the_market_version() {
    fn market(
        id: &str,
        version: Option<&str>,
        clob: &[&str],
        position: &[&str],
    ) -> serde_json::Value {
        let mut market = serde_json::json!({
            "id": id,
            "conditionId": format!("c{id}"),
            "question": format!("q{id}"),
            "marketMakerAddress": "0x1",
            "outcomes": "[\"Yes\",\"No\"]",
            "clobTokenIds": serde_json::to_string(clob).unwrap(),
            "positionIds": position,
        });
        if let Some(version) = version {
            market["version"] = version.into();
        }
        market
    }
    let served = serde_json::json!([
        market("1", Some("v1"), &["111", "112"], &["911", "912"]),
        market("2", Some("v2"), &["221", "222"], &["921", "922"]),
        market("3", None, &["331", "332"], &["931", "932"]),
        market("4", Some("v3"), &["441", "442"], &["941", "942"]),
    ]);

    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(served.to_string())
        .create_async()
        .await;

    let gamma = polyoxide_polymarket::gamma::Gamma::builder()
        .base_url(server.url())
        .build()
        .unwrap();
    let ids = discover_targets(&gamma, &DiscoverFilters::default())
        .await
        .unwrap();

    let clob = |index: usize| -> Vec<String> {
        serde_json::from_str(served[index]["clobTokenIds"].as_str().unwrap()).unwrap()
    };
    let position = |index: usize| -> Vec<String> {
        serde_json::from_value(served[index]["positionIds"].clone()).unwrap()
    };
    let mut expected = clob(0);
    expected.extend(position(1));
    expected.extend(clob(2));
    assert_eq!(ids, expected);
    assert_eq!(ids, ["111", "112", "921", "922", "331", "332"]);

    // Nothing of the `v2` market's `clobTokenIds`, and nothing of the `v3` market.
    for left_out in clob(1).into_iter().chain(clob(3)).chain(position(3)) {
        assert!(
            !ids.contains(&left_out),
            "{left_out} must not be discovered"
        );
    }
    mock.assert_async().await;
}
