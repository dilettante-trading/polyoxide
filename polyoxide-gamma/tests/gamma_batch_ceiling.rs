//! TEMP probe — binary-search the max number of IDs a single Gamma `/markets`
//! request will accept before something upstream (Cloudflare or Polymarket)
//! rejects it.
//!
//! Run:
//!   cargo test -p polyoxide-gamma --test gamma_batch_ceiling \
//!       -- --ignored --nocapture
//!
//! Probes two parameter shapes:
//!   - `id`             : numeric (i64 stringified, ~10 bytes each)
//!   - `condition_ids`  : 66-char 0x-prefixed hex (~66 bytes each)
//!
//! Fake but correctly-shaped IDs are fine — the server returns an empty array
//! for unknown IDs, so 200s mean "the URL was accepted" regardless of content.

use std::time::Duration;

const BASE_URL: &str = "https://gamma-api.polymarket.com/markets";
const UPPER_BOUND: usize = 5_000;
const INTER_REQUEST_PAUSE_MS: u64 = 150;

#[tokio::test]
#[ignore]
async fn find_gamma_batch_ceiling() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("reqwest client");

    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!("Gamma /markets batch-ID ceiling probe");
    eprintln!("═══════════════════════════════════════════════════════════════");

    let id_result = binary_search_ceiling(&client, "id", |n| {
        (0..n).map(|i| format!("{}", 1_000_000_000_i64 + i as i64)).collect()
    })
    .await;

    eprintln!();

    let cond_result = binary_search_ceiling(&client, "condition_ids", |n| {
        (0..n).map(|i| format!("0x{:064x}", i)).collect()
    })
    .await;

    eprintln!();
    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!("RESULTS");
    eprintln!("  id (numeric, ~10 B/entry):");
    eprintln!("    max accepted:  {} IDs", id_result.max_ok);
    eprintln!("    url length:    {} bytes at ceiling", id_result.url_len_at_ceiling);
    eprintln!("    fail status:   {}", id_result.fail_status);
    eprintln!("  condition_ids (hex, ~80 B/entry):");
    eprintln!("    max accepted:  {} IDs", cond_result.max_ok);
    eprintln!("    url length:    {} bytes at ceiling", cond_result.url_len_at_ceiling);
    eprintln!("    fail status:   {}", cond_result.fail_status);
    eprintln!("═══════════════════════════════════════════════════════════════");
}

struct Ceiling {
    max_ok: usize,
    url_len_at_ceiling: usize,
    fail_status: String,
}

async fn binary_search_ceiling(
    client: &reqwest::Client,
    param: &str,
    make: impl Fn(usize) -> Vec<String>,
) -> Ceiling {
    eprintln!("── Probing param={param} in [1, {UPPER_BOUND}]");

    let single = make(1);
    match send_n(client, param, &single).await {
        ProbeResult::Ok { .. } => eprintln!("   n=1 OK (sanity)"),
        ProbeResult::Fail { status, .. } => {
            eprintln!("   n=1 FAILED with {status} — aborting probe");
            return Ceiling {
                max_ok: 0,
                url_len_at_ceiling: 0,
                fail_status: status,
            };
        }
    }

    let upper = make(UPPER_BOUND);
    match send_n(client, param, &upper).await {
        ProbeResult::Ok { url_len } => {
            eprintln!("   n={UPPER_BOUND} OK — ceiling is above search space (url_len={url_len}B)");
            return Ceiling {
                max_ok: UPPER_BOUND,
                url_len_at_ceiling: url_len,
                fail_status: "<not triggered>".to_string(),
            };
        }
        ProbeResult::Fail { status, url_len } => {
            eprintln!("   n={UPPER_BOUND} FAIL {status} (url_len={url_len}B) — begin search");
        }
    }

    let mut lo = 1usize;
    let mut hi = UPPER_BOUND;
    let mut last_fail_status = String::new();
    let mut last_ok_url_len = 0usize;
    let mut last_fail_url_len = 0usize;

    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        tokio::time::sleep(Duration::from_millis(INTER_REQUEST_PAUSE_MS)).await;
        let ids = make(mid);
        match send_n(client, param, &ids).await {
            ProbeResult::Ok { url_len } => {
                eprintln!("   n={mid:<5} OK   (url_len={url_len}B)");
                lo = mid;
                last_ok_url_len = url_len;
            }
            ProbeResult::Fail { status, url_len } => {
                eprintln!("   n={mid:<5} FAIL {status} (url_len={url_len}B)");
                hi = mid;
                last_fail_status = status;
                last_fail_url_len = url_len;
            }
        }
    }

    eprintln!(
        "   → ceiling at n={lo} (url_len≈{last_ok_url_len}B); n={hi} fails at {last_fail_url_len}B"
    );

    Ceiling {
        max_ok: lo,
        url_len_at_ceiling: last_ok_url_len,
        fail_status: last_fail_status,
    }
}

enum ProbeResult {
    Ok { url_len: usize },
    Fail { status: String, url_len: usize },
}

async fn send_n(client: &reqwest::Client, param: &str, ids: &[String]) -> ProbeResult {
    let pairs: Vec<(&str, &str)> = ids.iter().map(|v| (param, v.as_str())).collect();
    let req = match client.get(BASE_URL).query(&pairs).build() {
        Ok(r) => r,
        Err(e) => {
            return ProbeResult::Fail {
                status: format!("build-error: {e}"),
                url_len: 0,
            };
        }
    };
    let url_len = req.url().as_str().len();
    match client.execute(req).await {
        Ok(resp) => {
            let code = resp.status();
            if code.is_success() {
                ProbeResult::Ok { url_len }
            } else {
                ProbeResult::Fail {
                    status: code.to_string(),
                    url_len,
                }
            }
        }
        Err(e) => ProbeResult::Fail {
            status: format!("transport: {e}"),
            url_len,
        },
    }
}
