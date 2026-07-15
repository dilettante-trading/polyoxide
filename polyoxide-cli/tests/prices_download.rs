use mockito::{Matcher, Server};
use polyoxide_cli::commands::clob::prices::download::DownloadArgs;
use polyoxide_clob::ClobBuilder;

fn base_args(out: &std::path::Path) -> DownloadArgs {
    DownloadArgs {
        token_ids: vec!["111".into(), "222".into()],
        input: None,
        discover: false,
        closed: None,
        open: None,
        min_volume: None,
        min_liquidity: None,
        tag_id: None,
        discover_limit: None,
        interval: "max".into(),
        fidelity: 1,
        start_ts: None,
        end_ts: None,
        out: out.to_path_buf(),
        format: polyoxide_cli::commands::clob::prices::types::OutputFormat::Csv,
        concurrency: 2,
        overwrite: false,
        fail_fast: false,
        dry_run: false,
    }
}

#[tokio::test]
async fn downloads_two_markets_and_writes_manifest() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .expect(2)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let args = base_args(dir.path());

    let summary = args.run_with_clients(&clob, None).await.unwrap();

    assert_eq!(summary.ok, 2);
    assert_eq!(summary.failed, 0);
    assert!(dir.path().join("111.csv").exists());
    assert!(dir.path().join("222.csv").exists());
    assert!(dir.path().join("manifest.jsonl").exists());
    mock.assert_async().await;
}

#[tokio::test]
async fn resume_skips_existing_files() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .expect(1)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("111.csv"), "token_id,timestamp,price\n").unwrap();

    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let summary = base_args(dir.path())
        .run_with_clients(&clob, None)
        .await
        .unwrap();

    assert_eq!(summary.ok, 1);
    assert_eq!(summary.skipped, 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn failed_market_is_isolated_and_recorded() {
    let mut server = Server::new_async().await;
    // 111 -> persistent 500 (fails after retries); 222 -> 200 ok.
    let _fail = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::UrlEncoded("market".into(), "111".into()))
        .with_status(500)
        .create_async()
        .await;
    let _ok = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::UrlEncoded("market".into(), "222".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let summary = base_args(dir.path())
        .run_with_clients(&clob, None)
        .await
        .unwrap();

    // 222 succeeded despite 111 failing (isolation).
    assert_eq!(summary.ok, 1);
    assert_eq!(summary.failed, 1);
    assert!(dir.path().join("222.csv").exists());
    assert!(!dir.path().join("111.csv").exists());
    let manifest = std::fs::read_to_string(dir.path().join("manifest.jsonl")).unwrap();
    assert!(manifest.contains("\"status\":\"failed\""));
    assert!(manifest.contains("\"token_id\":\"111\""));
}

#[tokio::test]
async fn fail_fast_returns_err_on_failure() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::Any)
        .with_status(500)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let clob = ClobBuilder::new().base_url(server.url()).build().unwrap();
    let mut args = base_args(dir.path());
    args.fail_fast = true;
    let result = args.run_with_clients(&clob, None).await;
    assert!(result.is_err());
}
