"""docs/MUTANTS.md's citations still point at the code they were proved on.

Each row of the ledger names a `file:line` (or `file:start-end`) where a rule
holds and the tests a mutant there must fail. A line that moves leaves the row
pointing at something else, and nothing would say so; this file records, for each
citation, a piece of the code it names, and fails when that line no longer holds it.
A range is checked at both ends. Re-prove the mutant whenever a row changes.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
LEDGER = REPO / "docs" / "MUTANTS.md"
CITATION = re.compile(r"(polyoxide[a-z-]*/[A-Za-z0-9_/.-]+\.rs):(\d+)(?:-(\d+))?")

# (file, line) -> a piece of that line.
SNIPPETS = {
    ("polyoxide-core/src/send.rs", 81): "self.throttle.observe(&charge, &meta, &info);",
    ("polyoxide-core/src/send.rs", 88): "self.throttle.hold(hold);",
    ("polyoxide-core/src/send.rs", 92): "Outcome::Retry(wait) if info.retries_left > 0 => {",
    ("polyoxide-core/src/send.rs", 94): "let sleep = floor.max(wait);",
    ("polyoxide-core/src/hooks.rs", 297): "hold: Some(schedule.retry_delay(0, response.retry_after())),",
    ("polyoxide-core/tests/send_loop.rs", 252): "async fn observe_sees_the_last_attempt(",
    ("polyoxide-core/tests/send_loop.rs", 278): "async fn a_zero_wait_still_sleeps_the_floor(",
    ("polyoxide-core/tests/send_loop.rs", 299): "async fn a_429_with_no_retry_left_still_holds(",
    ("polyoxide-core/tests/send_loop.rs", 322): "async fn the_429_hold_is_retry_delay_zero_not_the_attempts_wait(",
    ("polyoxide-core/tests/send_loop.rs", 580): "fn a_429_with_no_retry_left_is_fail_with_its_hold(",
    ("polyoxide-binance/src/usdm/request.rs", 130): "let cooldown = match (retry, asked) {",
    ("polyoxide-binance/src/usdm/request.rs", 143): "}",
    ("polyoxide-binance/tests/mock_api.rs", 616): "fn a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request(",
    ("polyoxide-binance/tests/mock_api.rs", 652): "fn a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute(",
    ("polyoxide-venue/src/retry_after.rs", 29): "if !secs.is_finite() || secs <= 0.0 {",
    ("polyoxide-venue/src/retry_after.rs", 35): "Some(wait).filter(|wait| !wait.is_zero())",
    ("polyoxide-venue/src/retry_after.rs", 45): "requested.map_or(computed, |requested| requested.max(computed))",
    ("polyoxide-venue/src/retry_after.rs", 55): "fn every_parser_row(",
    ("polyoxide-venue/src/retry_after.rs", 103): "fn a_requested_wait_only_ever_lengthens_the_backoff(",
    ("polyoxide-core/src/client.rs", 499): "fn retry_after_below_our_own_backoff_does_not_shorten_the_wait(",
    ("polyoxide-core/src/client.rs", 522): "fn retry_after_zero_still_backs_off_exponentially_across_attempts(",
    ("polyoxide-data/tests/mock_api.rs", 1509): "fn retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop(",
    ("polyoxide-data/tests/mock_api.rs", 1532): "fn a_429_makes_the_next_request_wait_even_though_it_never_saw_one(",
    ("polyoxide-data/tests/mock_api.rs", 1556): "fn a_429_on_the_pnl_host_holds_the_data_host(",
    ("polyoxide-perps/tests/mock_api.rs", 544): "fn a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff(",
    ("polyoxide-core/src/rate_limit.rs", 183): "fn quota(count: u32, period: Duration) -> Quota {",
    ("polyoxide-core/src/rate_limit.rs", 186): "}",
    ("polyoxide-core/src/rate_limit.rs", 263): "fn no_quota_admits_more_than_its_published_count_in_one_window(",
    ("polyoxide-core/src/rate_limit.rs", 280): "fn every_quota_reserves_headroom_below_the_published_count(",
    ("polyoxide-core/tests/polymarket_limits.rs", 20): "fn every_configured_bucket_satisfies_the_quota_it_publishes(",
    ("polyoxide-core/src/hold.rs", 72): "if slot.is_none_or(|current| until > current) {",
    ("polyoxide-core/src/hold.rs", 78): "pub async fn wait(&self) {",
    ("polyoxide-core/src/hold.rs", 135): "async fn a_hold_never_shortens(",
    ("polyoxide-core/src/hold.rs", 150): "async fn a_hold_extended_mid_wait_is_honoured_in_full(",
    ("polyoxide-core/src/rate_limit.rs", 577): "// AD-23: a hold set while this call waited on a bucket is honoured,",
    ("polyoxide-core/src/rate_limit.rs", 579): "self.inner.hold.wait().await;",
    ("polyoxide-core/src/rate_limit.rs", 830): "async fn a_hold_set_during_a_bucket_wait_is_honoured(",
    ("polyoxide-core/src/hold.rs", 91): "tokio::time::sleep_until(deadline).await;",
    ("polyoxide-core/src/hold.rs", 93): "}",
    ("polyoxide-core/tests/polymarket_limits.rs", 1035): "fn a_shorter_cooldown_never_cuts_a_longer_one_short(",
    ("polyoxide-core/tests/polymarket_limits.rs", 1053): "fn a_cooldown_extended_mid_wait_is_honoured_in_full(",
    ("polyoxide-core/src/signer_limit.rs", 276): "let capacity = tier.burst(bucket).max(1);",
    ("polyoxide-core/src/signer_limit.rs", 523): "async fn a_batch_within_capacity_is_admitted(",
    ("polyoxide-core/src/signer_limit.rs", 532): "async fn adopting_a_higher_tier_admits_a_batch_that_was_impossible(",
    ("polyoxide-core/src/signer_limit.rs", 548): "async fn the_order_and_cancel_buckets_are_independent(",
    ("polyoxide-core/src/signer_limit.rs", 570): "async fn batch_cost_is_charged_in_full_not_as_one_request(",
    ("polyoxide-core/src/rate_limit.rs", 208): "let target = count.saturating_sub(count.div_ceil(RESERVED_FRACTION));",
    ("polyoxide-core/src/rate_limit.rs", 216): "const RESERVED_FRACTION: u32 = 10;",
    ("polyoxide-core/src/rate_limit.rs", 303): "fn a_published_150_per_10s_admits_135_per_window(",
    ("polyoxide-binance/src/usdm/request.rs", 107): "self.budget.begin_cooldown(ban);",
    ("polyoxide-binance/tests/mock_api.rs", 409): "async fn a_418_is_not_retried_and_holds_the_next_request(",
    ("polyoxide-clob/src/error.rs", 89): "let m = message.to_ascii_lowercase();",
    ("polyoxide-clob/src/error.rs", 92): 'if m.contains("fak order") && (m.contains("no match")',
    ("polyoxide-clob/src/error.rs", 110): "ApiError::Validation(msg) => {",
    ("polyoxide-clob/src/error.rs", 113): "other => Self::Api(other),",
    ("polyoxide-clob/src/error.rs", 314): "fn test_classify_recognizes_verbatim_venue_messages(",
    ("polyoxide-clob/src/error.rs", 326): "fn test_classify_preserves_message_verbatim(",
    ("polyoxide-clob/src/error.rs", 334): "fn test_classify_is_case_insensitive(",
    ("polyoxide-clob/src/error.rs", 347): "fn test_classify_tolerates_curly_apostrophe_in_fok_message(",
    ("polyoxide-clob/src/error.rs", 358): "fn test_classify_does_not_capture_neighbouring_400s(",
    ("polyoxide-clob/src/error.rs", 389): "fn test_classify_requires_both_tokens(",
    ("polyoxide-clob/tests/mock_api.rs", 3365): "fn fak_unmatched_maps_to_typed_error_not_generic_validation(",
    ("polyoxide-clob/tests/mock_api.rs", 3399): "fn fok_unfilled_maps_to_typed_error_not_generic_validation(",
    ("polyoxide-clob/tests/mock_api.rs", 3462): "fn fak_prose_on_non_400_status_is_not_reclassified(",
    ("polyoxide-clob/src/request.rs", 255): "http_client.note_rate_limited(status, retry_after.as_deref());",
    ("polyoxide-relay/src/client.rs", 293): ".note_rate_limited(resp.status(), retry_after.as_deref());",
    ("polyoxide-relay/src/client.rs", 392): ".note_rate_limited(resp.status(), retry_after.as_deref());",
    ("polyoxide-relay/src/client.rs", 1836): ".note_rate_limited(status, retry_after.as_deref());",
}


def cited() -> set[tuple[str, int]]:
    lines = set()
    for match in CITATION.finditer(LEDGER.read_text()):
        lines.add((match[1], int(match[2])))
        if match[3]:
            lines.add((match[1], int(match[3])))
    return lines


def test_every_citation_has_a_recorded_snippet_and_every_snippet_is_cited() -> None:
    assert cited() == set(SNIPPETS)


@pytest.mark.parametrize(("path", "line"), sorted(SNIPPETS))
def test_each_cited_line_still_holds_its_code(path: str, line: int) -> None:
    source = (REPO / path).read_text().splitlines()
    assert line <= len(source), f"{path} has only {len(source)} lines"
    assert SNIPPETS[(path, line)] in source[line - 1], (
        f"{path}:{line} is now {source[line - 1].strip()!r}; move the MUTANTS.md row "
        f"and prove its mutant again")
