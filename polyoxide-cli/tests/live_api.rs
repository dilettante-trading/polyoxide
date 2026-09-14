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

// ── data (Data API v2) ───────────────────────────────────────────────
//
// Inputs are chosen live, never hardcoded: a wallet and market from the bare
// trade feed. Every listing asks for two rows, to keep the load light.

mod data_v2 {
    use clap::Parser;
    use polyoxide_cli::commands::DataCommand;
    use polyoxide_data::DataApi;
    use serde_json::Value;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        data: DataCommand,
    }

    /// Runs a `data` command against the live host and returns its stdout
    /// and stderr.
    async fn data(args: &[&str]) -> (String, String) {
        let cli = Cli::try_parse_from(std::iter::once("data").chain(args.iter().copied()))
            .unwrap_or_else(|e| panic!("{args:?}: {e}"));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        cli.data
            .run_with(&DataApi::new().unwrap(), &mut out, &mut err)
            .await
            .unwrap_or_else(|e| panic!("{args:?}: {e:?}"));
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn json(args: &[&str], out: &str) -> Value {
        serde_json::from_str(out).unwrap_or_else(|e| panic!("{args:?} printed non-JSON: {e}"))
    }

    #[tokio::test]
    #[ignore = "hits the real Polymarket API"]
    async fn live_data_commands_read_v2() {
        let args = ["trades", "list", "--limit", "1"];
        let feed = json(&args, &data(&args).await.0);
        let trade = &feed["data"][0];
        let wallet = trade["proxy_wallet"].as_str().expect("snake_case rows");
        let condition = trade["condition_id"].as_str().expect("snake_case rows");

        for args in [
            &["activity", "--user", wallet, "--limit", "2"][..],
            &["positions", "--user", wallet, "list", "--limit", "2"][..],
            &[
                "positions",
                "--user",
                wallet,
                "list",
                "--status",
                "closed",
                "--limit",
                "2",
            ][..],
            &["positions", "--user", wallet, "value"][..],
            &["holders", "--condition", condition, "--limit", "2"][..],
            &["open-interest", "--condition", condition][..],
            &["builders", "leaderboard", "--limit", "2"][..],
            &["builders", "volume", "--limit", "2"][..],
        ] {
            json(args, &data(args).await.0);
        }

        let args = ["traded", "--user", wallet];
        let stats = json(&args, &data(&args).await.0);
        assert_eq!(
            stats["proxy_wallet"].as_str().map(str::to_lowercase),
            Some(wallet.to_lowercase()),
            "a wallet that just traded has stats: {stats}"
        );
        assert!(stats["trades"].is_u64(), "{stats}");
    }

    #[tokio::test]
    #[ignore = "hits the real Polymarket API"]
    async fn live_data_walk_stops_at_max_pages_and_resumes_from_its_cursor() {
        let args = [
            "trades",
            "list",
            "--limit",
            "2",
            "--all",
            "--max-pages",
            "2",
        ];
        let (rows, err) = data(&args).await;
        assert_eq!(rows.lines().count(), 4, "two pages of two rows");
        for row in rows.lines() {
            json(&args, row);
        }
        let cursor = err
            .strip_prefix("next_cursor: ")
            .unwrap_or_else(|| panic!("no resume cursor on stderr: {err:?}"))
            .trim_end();

        let args = ["trades", "list", "--limit", "2", "--cursor", cursor];
        let resumed = json(&args, &data(&args).await.0);
        assert_eq!(resumed["data"].as_array().map(Vec::len), Some(2));
    }
}
