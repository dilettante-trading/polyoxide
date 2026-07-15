//! Live integration tests. Hit the real Polymarket API; skipped in CI.
//! Run with: `cargo test -p polyoxide-cli --test live_api -- --ignored`

use polyoxide_cli::commands::clob::prices::download::DownloadArgs;
use polyoxide_cli::commands::clob::prices::types::OutputFormat;
use polyoxide_clob::Clob;

#[tokio::test]
#[ignore = "hits the real Polymarket API"]
async fn live_download_one_market() {
    // A known liquid token id; update if it resolves empty.
    let token_id = "71321045679252212594626385532706912750332728571942532289631379312455583992563";
    let dir = tempfile::tempdir().unwrap();

    let args = DownloadArgs {
        token_ids: vec![token_id.into()],
        input: None,
        discover: false,
        closed: None,
        open: None,
        min_volume: None,
        min_liquidity: None,
        tag_id: None,
        discover_limit: None,
        interval: "1d".into(),
        fidelity: 60,
        start_ts: None,
        end_ts: None,
        out: dir.path().to_path_buf(),
        format: OutputFormat::Csv,
        concurrency: 1,
        overwrite: false,
        fail_fast: false,
        dry_run: false,
    };

    let clob = Clob::public();
    let summary = args.run_with_clients(&clob, None).await.unwrap();
    assert_eq!(summary.failed, 0);
    assert!(dir.path().join(format!("{token_id}.csv")).exists());
}
