use std::time::Duration;

use governor::DefaultDirectRateLimiter;
use polyoxide_clob::{Clob, ClobError, PriceHistoryPoint, PricesHistoryQuery};

/// Fetch a single market's price history, waiting for a rate-limit permit
/// before each attempt and retrying transient failures with exponential
/// backoff (200ms, 400ms, 800ms, ...).
///
/// The typed client abstracts away HTTP status codes and the `Retry-After`
/// header, so every error is treated as retryable up to `max_retries`. This is
/// a deliberate simplification over per-status handling (see spec).
#[allow(dead_code)] // used by the download orchestration in a later task
pub async fn fetch_one(
    clob: &Clob,
    token_id: &str,
    query: &PricesHistoryQuery,
    limiter: &DefaultDirectRateLimiter,
    max_retries: u32,
) -> Result<Vec<PriceHistoryPoint>, ClobError> {
    let mut attempt = 0;
    loop {
        limiter.until_ready().await;
        match clob
            .markets()
            .prices_history_with(token_id, query)
            .send()
            .await
        {
            Ok(resp) => return Ok(resp.history),
            Err(e) => {
                if attempt >= max_retries {
                    return Err(e);
                }
                // Clamp the shift and saturate so a large max_retries can't
                // overflow (panic in debug) or wrap (hang in release); cap at 30s.
                let backoff_ms = 200u64.saturating_mul(1u64 << attempt.min(10)).min(30_000);
                let backoff = Duration::from_millis(backoff_ms);
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
        }
    }
}
