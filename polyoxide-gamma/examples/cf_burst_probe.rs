//! Temporary probe — burst-hit Gamma `/markets` to empirically observe what
//! Cloudflare / Polymarket actually send when rate limits kick in.
//!
//! Exists to validate assumptions about our `Retry-After` parser. Delete once
//! the findings are recorded.
//!
//! Run:
//! ```sh
//! cargo run -p polyoxide-gamma --example cf_burst_probe
//! ```
//!
//! Behaviour:
//!   - Bypasses the SDK's rate limiter (uses `reqwest::Client` directly).
//!   - Fires requests in concurrent batches until the server returns non-2xx.
//!   - Dumps the triggering response's status, key headers, and body preview.
//!   - Hard-caps at `MAX_REQUESTS` to stay well-behaved if no limit trips.

use std::time::{Duration, Instant};

const URL: &str = "https://gamma-api.polymarket.com/markets?limit=1";
const MAX_REQUESTS: u32 = 2_000;
const BATCH_SIZE: u32 = 50;
const SAMPLE_EVERY: u32 = 250;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client");

    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!("CF burst probe → {URL}");
    eprintln!("  cap={MAX_REQUESTS}, concurrency={BATCH_SIZE}, stop-on-first-fail");
    eprintln!("═══════════════════════════════════════════════════════════════");

    let start = Instant::now();
    let mut sent: u32 = 0;
    let mut successes: u32 = 0;
    let mut shown_first_sample = false;

    'outer: loop {
        if sent >= MAX_REQUESTS {
            eprintln!();
            eprintln!("Reached MAX_REQUESTS ({MAX_REQUESTS}) without triggering throttle.");
            break;
        }

        let remaining = (MAX_REQUESTS - sent).min(BATCH_SIZE);
        let handles: Vec<_> = (0..remaining)
            .map(|i| {
                let client = client.clone();
                let idx = sent + i;
                tokio::spawn(async move {
                    let t0 = Instant::now();
                    let res = client.get(URL).send().await;
                    (idx, t0.elapsed(), res)
                })
            })
            .collect();

        for h in handles {
            sent += 1;
            let (idx, elapsed, res) = h.await.expect("task panicked");
            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        eprintln!();
                        eprintln!("▼▼▼ First non-2xx at request #{idx} ▼▼▼");
                        eprintln!(
                            "  elapsed since probe start: {:.2}s",
                            start.elapsed().as_secs_f64()
                        );
                        eprintln!("  request latency:           {}ms", elapsed.as_millis());
                        dump(resp, true).await;
                        break 'outer;
                    }
                    successes += 1;
                    if !shown_first_sample || successes.is_multiple_of(SAMPLE_EVERY) {
                        shown_first_sample = true;
                        eprintln!();
                        eprintln!(
                            "── Sample 200 at request #{idx} (latency {}ms)",
                            elapsed.as_millis()
                        );
                        dump(resp, false).await;
                    }
                }
                Err(e) => {
                    eprintln!();
                    eprintln!("▼▼▼ Transport error at request #{idx}: {e}");
                    break 'outer;
                }
            }
        }
    }

    let total = start.elapsed();
    eprintln!();
    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!(
        "Summary: {sent} sent, {successes} 2xx over {:.2}s ({:.1} req/s)",
        total.as_secs_f64(),
        sent as f64 / total.as_secs_f64().max(0.001)
    );
    eprintln!("═══════════════════════════════════════════════════════════════");
}

async fn dump(resp: reqwest::Response, include_body: bool) {
    const RELEVANT: &[&str] = &[
        "retry-after",
        "cf-ray",
        "cf-cache-status",
        "cf-mitigated",
        "cf-chl-bypass",
        "server",
        "content-type",
        "date",
        "x-ratelimit-limit",
        "x-ratelimit-remaining",
        "x-ratelimit-reset",
    ];

    let status = resp.status();
    eprintln!("  status: {status}");
    eprintln!("  headers:");
    for (name, val) in resp.headers() {
        if RELEVANT
            .iter()
            .any(|k| name.as_str().eq_ignore_ascii_case(k))
        {
            eprintln!("    {name}: {}", val.to_str().unwrap_or("<non-utf8>"));
        }
    }

    if include_body {
        let body = resp.text().await.unwrap_or_default();
        let preview: String = body.chars().take(500).collect();
        eprintln!("  body ({}B total, first 500 chars):", body.len());
        for line in preview.lines().take(20) {
            eprintln!("    | {line}");
        }
    }
}
