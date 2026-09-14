//! The `data` commands against a mock Data API v2 server.
//!
//! Each test parses real command-line arguments, runs the command against a
//! mock host serving a response captured from the live API, and checks the
//! query the command sent and what it wrote. Parsing tests alone cannot see a
//! flag that parses but never reaches the request, or one that parses and then
//! panics when clap hands it to the command.

use std::sync::{Arc, Mutex};

use clap::Parser;
use mockito::{Matcher, Server, ServerGuard};
use polyoxide_cli::commands::DataCommand;
use polyoxide_data::DataApi;
use serde_json::Value;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    data: DataCommand,
}

type Pairs = Vec<(String, String)>;

/// A response captured from the live API by `polyoxide-data`.
fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../polyoxide-data/tests/fixtures/v2/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// What a command returned and wrote.
struct Run {
    result: color_eyre::Result<()>,
    out: String,
    err: String,
}

impl Run {
    fn expect_ok(&self) {
        if let Err(error) = &self.result {
            panic!("command failed: {error:?}\nstderr: {}", self.err);
        }
    }

    /// Each stdout line, parsed as JSON.
    fn rows(&self) -> Vec<Value> {
        self.out
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

async fn run(server: &ServerGuard, args: &[&str]) -> Run {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = execute(server, args, &mut out, &mut err).await;
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

async fn execute(
    server: &ServerGuard,
    args: &[&str],
    out: &mut Vec<u8>,
    err: &mut Vec<u8>,
) -> color_eyre::Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("data").chain(args.iter().copied()))?;
    let data = DataApi::builder().base_url(server.url()).build()?;
    cli.data.run_with(&data, out, err).await
}

/// Decodes a query string into sorted pairs, so tests do not depend on the
/// order the builder appends parameters in.
fn query_pairs(path_and_query: &str) -> Pairs {
    let Some((_, query)) = path_and_query.split_once('?') else {
        return Vec::new();
    };
    let mut pairs: Pairs = query
        .split('&')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect();
    pairs.sort();
    pairs
}

fn decode(component: &str) -> String {
    let bytes = component.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                out.push(u8::from_str_radix(&component[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

fn pairs(expected: &[(&str, &str)]) -> Pairs {
    let mut pairs: Pairs = expected
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    pairs.sort();
    pairs
}

/// Serves `path`, answering each request with `respond(query)` and recording
/// every query in the order received.
async fn serve<F>(server: &mut ServerGuard, path: &str, respond: F) -> Arc<Mutex<Vec<Pairs>>>
where
    F: Fn(&Pairs) -> String + Send + Sync + 'static,
{
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock()
                .unwrap()
                .push(query_pairs(request.path_and_query()));
            true
        })
        .with_status(200)
        .with_body_from_request(move |request| {
            respond(&query_pairs(request.path_and_query())).into_bytes()
        })
        .create_async()
        .await;
    seen
}

/// Runs `args` against a host serving `body` on `path`, requires success, and
/// returns the query of the single request sent along with the run.
async fn sent(path: &str, body: &str, args: &[&str]) -> (Pairs, Run) {
    let mut server = Server::new_async().await;
    let body = body.to_owned();
    let seen = serve(&mut server, path, move |_| body.clone()).await;
    let run = run(&server, args).await;
    run.expect_ok();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{args:?} sent {seen:?}");
    (seen[0].clone(), run)
}

/// The captured two-row trades page, with its cursor replaced so a walk can be
/// steered.
fn trades_page(next_cursor: Option<&str>) -> String {
    let mut page: Value = serde_json::from_str(&fixture("trades")).unwrap();
    page["pagination"]["has_more"] = Value::Bool(next_cursor.is_some());
    page["pagination"]["next_cursor"] = next_cursor.map_or(Value::Null, Value::from);
    page.to_string()
}

fn cursor(query: &Pairs) -> Option<String> {
    query
        .iter()
        .find(|(key, _)| key == "cursor")
        .map(|(_, value)| value.clone())
}

fn cursors(seen: &Mutex<Vec<Pairs>>) -> Vec<Option<String>> {
    seen.lock().unwrap().iter().map(cursor).collect()
}

fn some(cursor: &str) -> Option<String> {
    Some(cursor.to_owned())
}

fn without_cursor(query: &Pairs) -> Pairs {
    query
        .iter()
        .filter(|(key, _)| key != "cursor")
        .cloned()
        .collect()
}

/// Serves a three-page trade feed: no cursor, then `c1`, then `c2`.
async fn three_pages(server: &mut ServerGuard) -> Arc<Mutex<Vec<Pairs>>> {
    serve(server, "/v2/trades", |query| {
        match cursor(query).as_deref() {
            None => trades_page(Some("c1")),
            Some("c1") => trades_page(Some("c2")),
            Some("c2") => trades_page(None),
            Some(other) => panic!("unexpected cursor {other}"),
        }
    })
    .await
}

// ── trades ───────────────────────────────────────────────────────────

#[tokio::test]
async fn trades_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/trades",
        &fixture("trades"),
        &[
            "trades",
            "list",
            "--user",
            "0xu",
            "--market",
            "0xa,0xb",
            "--event-id",
            "7,8",
            "--side",
            "sell",
            "--taker-only",
            "false",
            "--filter-type",
            "cash",
            "--filter-amount",
            "5",
            "--start",
            "100",
            "--end",
            "200",
            "--limit",
            "3",
        ],
    )
    .await;

    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("condition", "0xa,0xb"),
            ("event_id", "7,8"),
            ("side", "SELL"),
            ("taker_only", "false"),
            ("filter_type", "CASH"),
            ("filter_amount", "5"),
            ("start", "100"),
            ("end", "200"),
            ("limit", "3"),
        ])
    );
}

#[tokio::test]
async fn trades_defaults_send_no_offset() {
    let (query, _) = sent("/v2/trades", &fixture("trades"), &["trades", "list"]).await;
    assert_eq!(query, pairs(&[("taker_only", "true"), ("limit", "100")]));
}

#[tokio::test]
async fn a_single_page_prints_the_envelope_as_received() {
    let (_, run) = sent("/v2/trades", &fixture("trades"), &["trades", "list"]).await;

    let printed: Value = serde_json::from_str(&run.out).unwrap();
    let received: Value = serde_json::from_str(&fixture("trades")).unwrap();
    assert_eq!(printed["pagination"], received["pagination"]);
    assert_eq!(
        printed["data"][0]["proxy_wallet"], received["data"][0]["proxy_wallet"],
        "rows keep v2's snake_case field names"
    );
    assert_eq!(run.err, "");
}

// ── paging ───────────────────────────────────────────────────────────

#[tokio::test]
async fn cursor_resumes_a_single_page() {
    let (query, _) = sent(
        "/v2/trades",
        &fixture("trades"),
        &["trades", "list", "--cursor", "c7"],
    )
    .await;
    assert_eq!(cursor(&query), some("c7"));
}

#[tokio::test]
async fn all_writes_every_row_of_every_page_as_jsonl() {
    let mut server = Server::new_async().await;
    let seen = three_pages(&mut server).await;

    let run = run(
        &server,
        &["trades", "list", "--user", "0xu", "--limit", "2", "--all"],
    )
    .await;
    run.expect_ok();

    let rows = run.rows();
    assert_eq!(rows.len(), 6, "three pages of two rows");
    assert!(rows.iter().all(|row| row["proxy_wallet"].is_string()));
    assert_eq!(
        run.err, "",
        "a walk that reaches the end has nothing to resume"
    );

    assert_eq!(cursors(&seen), [None, some("c1"), some("c2")]);
    let seen = seen.lock().unwrap();
    let first = without_cursor(&seen[0]);
    assert_eq!(
        first,
        pairs(&[("user", "0xu"), ("taker_only", "true"), ("limit", "2")])
    );
    for (i, query) in seen.iter().enumerate() {
        assert_eq!(
            without_cursor(query),
            first,
            "page {} changed filters",
            i + 1
        );
    }
}

#[tokio::test]
async fn max_pages_stops_the_walk_and_prints_the_cursor_to_resume_from() {
    let mut server = Server::new_async().await;
    let seen = three_pages(&mut server).await;

    let run = run(&server, &["trades", "list", "--all", "--max-pages", "2"]).await;
    run.expect_ok();

    assert_eq!(run.rows().len(), 4);
    assert_eq!(run.err, "next_cursor: c2\n");
    assert_eq!(
        cursors(&seen),
        [None, some("c1")],
        "the third page is never fetched"
    );
}

#[tokio::test]
async fn all_starts_from_the_given_cursor() {
    let mut server = Server::new_async().await;
    let seen = three_pages(&mut server).await;

    let run = run(&server, &["trades", "list", "--cursor", "c1", "--all"]).await;
    run.expect_ok();

    assert_eq!(run.rows().len(), 4);
    assert_eq!(cursors(&seen), [some("c1"), some("c2")]);
}

#[tokio::test]
async fn a_failed_page_fails_the_walk_and_prints_the_cursor_to_retry() {
    let mut server = Server::new_async().await;
    let fails = |request: &mockito::Request| {
        cursor(&query_pairs(request.path_and_query())).as_deref() == Some("c1")
    };
    server
        .mock("GET", "/v2/trades")
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |request| if fails(request) { 500 } else { 200 })
        .with_body_from_request(move |request| {
            if fails(request) {
                br#"{"error":"boom","code":"internal","retryable":false,"trace_id":"t-500"}"#
                    .to_vec()
            } else {
                trades_page(Some("c1")).into_bytes()
            }
        })
        .expect(2)
        .create_async()
        .await;

    let run = run(&server, &["trades", "list", "--all"]).await;

    assert!(
        run.result.is_err(),
        "a walk that loses a page must not exit 0"
    );
    assert_eq!(run.rows().len(), 2, "rows before the failure are kept");
    assert_eq!(run.err, "next_cursor: c1\n");
}

#[tokio::test]
async fn offset_is_refused_with_a_pointer_to_cursor() {
    let server = Server::new_async().await;
    for args in [
        &["trades", "list", "--offset", "100"][..],
        &["trades", "list", "-o", "100"][..],
        &["activity", "--user", "0xu", "--offset", "100"][..],
        &["positions", "--user", "0xu", "list", "--offset", "100"][..],
        &["holders", "--condition", "0xc", "--offset", "100"][..],
    ] {
        let run = run(&server, args).await;
        let message = run.result.unwrap_err().to_string();
        assert!(message.contains("--cursor"), "{args:?}: {message}");
    }
}

#[tokio::test]
async fn max_pages_without_all_is_refused() {
    let server = Server::new_async().await;
    let run = run(&server, &["trades", "list", "--max-pages", "2"]).await;
    assert!(run.result.is_err());
}

// ── positions and activity ───────────────────────────────────────────

#[tokio::test]
async fn positions_list_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/positions",
        &fixture("positions_closed"),
        &[
            "positions",
            "--user",
            "0xu",
            "list",
            "--condition",
            "0xa,0xb",
            "--event-id",
            "7",
            "--status",
            "closed",
            "--title",
            "rain",
            "--filter-type",
            "tokens",
            "--filter-amount",
            "2.5",
            "--include-archived",
            "--sort-by",
            "realized-pnl",
            "--sort-direction",
            "asc",
            "--start",
            "100",
            "--end",
            "200",
            "--limit",
            "4",
        ],
    )
    .await;

    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("condition", "0xa,0xb"),
            ("event_id", "7"),
            ("status", "CLOSED"),
            ("title", "rain"),
            ("filter_type", "TOKENS"),
            ("filter_amount", "2.5"),
            ("include_archived", "true"),
            ("sort_by", "REALIZED_PNL"),
            ("sort_direction", "ASC"),
            ("start", "100"),
            ("end", "200"),
            ("limit", "4"),
        ])
    );
}

#[tokio::test]
async fn positions_list_defaults_to_open_positions_for_the_user() {
    let (query, _) = sent(
        "/v2/positions",
        &fixture("positions"),
        &["positions", "--user", "0xu", "list"],
    )
    .await;
    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("status", "OPEN"),
            ("sort_direction", "DESC"),
            ("limit", "100"),
        ])
    );
}

#[tokio::test]
async fn positions_value_scopes_to_conditions() {
    let (query, _) = sent(
        "/v2/value",
        &fixture("value"),
        &["positions", "--user", "0xu", "value", "--market", "0xa,0xb"],
    )
    .await;
    assert_eq!(query, pairs(&[("user", "0xu"), ("condition", "0xa,0xb")]));
}

#[tokio::test]
async fn positions_closed_names_its_replacement() {
    let server = Server::new_async().await;
    let run = run(&server, &["positions", "--user", "0xu", "closed"]).await;
    let message = run.result.unwrap_err().to_string();
    assert!(message.contains("--status closed"), "{message}");
}

#[tokio::test]
async fn activity_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/activity",
        &fixture("activity"),
        &[
            "activity",
            "--user",
            "0xu",
            "--condition",
            "0xa",
            "--event-id",
            "7",
            "--activity-type",
            "trade,tip",
            "--side",
            "buy",
            "--start",
            "100",
            "--end",
            "200",
            "--include-deposits-withdrawals",
            "--sort-direction",
            "asc",
            "--limit",
            "5",
        ],
    )
    .await;

    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("condition", "0xa"),
            ("event_id", "7"),
            ("type", "TRADE,TIP"),
            ("side", "BUY"),
            ("start", "100"),
            ("end", "200"),
            ("exclude_deposits_withdrawals", "false"),
            ("sort_direction", "ASC"),
            ("limit", "5"),
        ])
    );
}

#[tokio::test]
async fn positions_activity_uses_the_positions_user() {
    let (query, _) = sent(
        "/v2/activity",
        &fixture("activity"),
        &["positions", "--user", "0xu", "activity"],
    )
    .await;
    assert_eq!(
        query,
        pairs(&[
            ("user", "0xu"),
            ("sort_direction", "DESC"),
            ("limit", "100")
        ]),
        "deposits stay at the API's default of hidden unless asked for"
    );
}

// ── holders, open interest and live volume ───────────────────────────

#[tokio::test]
async fn holders_flags_reach_their_v2_parameters() {
    let (query, _) = sent(
        "/v2/holders",
        &fixture("holders_pnl"),
        &[
            "holders",
            "--market",
            "0xc",
            "--min-balance",
            "2.5",
            "--include-pnl",
            "--limit",
            "10",
        ],
    )
    .await;
    assert_eq!(
        query,
        pairs(&[
            ("condition", "0xc"),
            ("min_balance", "2.5"),
            ("include_pnl", "true"),
            ("limit", "10"),
        ])
    );
}

#[tokio::test]
async fn open_interest_is_global_without_markets() {
    let (query, _) = sent(
        "/v2/oi",
        &fixture("open_interest_global"),
        &["open-interest"],
    )
    .await;
    assert_eq!(query, pairs(&[]));
}

#[tokio::test]
async fn open_interest_scopes_to_markets() {
    let (query, _) = sent(
        "/v2/oi",
        &fixture("open_interest"),
        &["open-interest", "--market", "0xa,0xb"],
    )
    .await;
    assert_eq!(query, pairs(&[("condition", "0xa,0xb")]));
}

#[tokio::test]
async fn live_volume_sends_every_event_id() {
    let (query, _) = sent(
        "/v2/live-volume",
        &fixture("live_volume"),
        &["live-volume", "--event-id", "1,2"],
    )
    .await;
    assert_eq!(query, pairs(&[("event_id", "1,2")]));
}
