use mockito::{Matcher, Server};
use polyoxide_cli::commands::clob::prices::select::{discover_targets, DiscoverFilters};

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

    let gamma = polyoxide_gamma::Gamma::builder()
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
