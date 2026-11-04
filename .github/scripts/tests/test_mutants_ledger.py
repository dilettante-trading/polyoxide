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
    ("polyoxide-core/src/send.rs", 84): "self.throttle.observe(&charge, &meta, &info);",
    ("polyoxide-core/src/send.rs", 91): "self.throttle.hold(hold);",
    ("polyoxide-core/src/send.rs", 95): "Outcome::Retry(wait) if info.retries_left > 0 => {",
    ("polyoxide-core/src/send.rs", 97): "let sleep = floor.max(wait);",
    ("polyoxide-core/src/hooks.rs", 297): "hold: Some(schedule.retry_delay(0, response.retry_after())),",
    ("polyoxide-polymarket/tests/send_loop.rs", 252): "async fn observe_sees_the_last_attempt(",
    ("polyoxide-polymarket/tests/send_loop.rs", 278): "async fn a_zero_wait_still_sleeps_the_floor(",
    ("polyoxide-polymarket/tests/send_loop.rs", 299): "async fn a_429_with_no_retry_left_still_holds(",
    ("polyoxide-polymarket/tests/send_loop.rs", 322): "async fn the_429_hold_is_retry_delay_zero_not_the_attempts_wait(",
    ("polyoxide-polymarket/tests/send_loop.rs", 580): "fn a_429_with_no_retry_left_is_fail_with_its_hold(",
    ("polyoxide-binance/src/shared/policy.rs", 47): "hold: Some(asked.unwrap_or(until_next_minute)),",
    ("polyoxide-binance/src/shared/policy.rs", 51): "hold: Some(asked.unwrap_or(DEFAULT_BAN)),",
    ("polyoxide-binance/src/usdm/policy.rs", 160): "fn a_418_without_retry_after_holds_two_minutes(",
    ("polyoxide-binance/tests/mock_usdm.rs", 707): "async fn every_usdm_on_one_budget_is_held_by_one_ban(",
    ("polyoxide-binance/tests/mock_usdm.rs", 616): "fn a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request(",
    ("polyoxide-binance/tests/mock_usdm.rs", 652): "fn a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute(",
    ("polyoxide-venue/src/retry_after.rs", 29): "if !secs.is_finite() || secs <= 0.0 {",
    ("polyoxide-venue/src/retry_after.rs", 35): "Some(wait).filter(|wait| !wait.is_zero())",
    ("polyoxide-venue/src/retry_after.rs", 45): "requested.map_or(computed, |requested| requested.max(computed))",
    ("polyoxide-venue/src/retry_after.rs", 55): "fn every_parser_row(",
    ("polyoxide-venue/src/retry_after.rs", 103): "fn a_requested_wait_only_ever_lengthens_the_backoff(",
    ("polyoxide-core/src/client.rs", 353): "fn retry_after_below_our_own_backoff_does_not_shorten_the_wait(",
    ("polyoxide-core/src/client.rs", 372): "fn retry_after_zero_still_backs_off_exponentially_across_attempts(",
    ("polyoxide-polymarket/tests/mock_data.rs", 720): "fn retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop(",
    ("polyoxide-polymarket/tests/mock_data.rs", 747): "fn a_429_makes_the_next_request_wait_even_though_it_never_saw_one(",
    ("polyoxide-polymarket/tests/mock_perps.rs", 544): "fn a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff(",
    ("polyoxide-core/src/rate_limit.rs", 236): "fn quota(count: u32, period: Duration) -> Quota {",
    ("polyoxide-core/src/rate_limit.rs", 239): "}",
    ("polyoxide-core/src/rate_limit.rs", 316): "fn no_quota_admits_more_than_its_published_count_in_one_window(",
    ("polyoxide-core/src/rate_limit.rs", 333): "fn every_quota_reserves_headroom_below_the_published_count(",
    ("polyoxide-polymarket/tests/polymarket_limits.rs", 21): "fn every_configured_bucket_satisfies_the_quota_it_publishes(",
    ("polyoxide-core/src/hold.rs", 99): "if slot.is_none_or(|current| until > current) {",
    ("polyoxide-core/src/hold.rs", 105): "pub async fn wait(&self) {",
    ("polyoxide-core/src/hold.rs", 165): "async fn a_hold_never_shortens(",
    ("polyoxide-core/src/hold.rs", 180): "async fn a_hold_extended_mid_wait_is_honoured_in_full(",
    ("polyoxide-core/src/rate_limit.rs", 657): "// AD-23: a hold set while this call waited on a bucket is honoured,",
    ("polyoxide-core/src/rate_limit.rs", 659): "self.inner.hold.wait().await;",
    ("polyoxide-core/src/rate_limit.rs", 910): "async fn a_hold_set_during_a_bucket_wait_is_honoured(",
    ("polyoxide-core/src/hold.rs", 118): "self.inner.runtime.sleep_until(deadline).await;",
    ("polyoxide-core/src/hold.rs", 120): "}",
    ("polyoxide-polymarket/tests/polymarket_limits.rs", 1004): "fn a_shorter_cooldown_never_cuts_a_longer_one_short(",
    ("polyoxide-polymarket/tests/polymarket_limits.rs", 1022): "fn a_cooldown_extended_mid_wait_is_honoured_in_full(",
    ("polyoxide-polymarket/src/shared/signer_limit.rs", 278): "let capacity = tier.burst(bucket).max(1);",
    ("polyoxide-polymarket/src/shared/signer_limit.rs", 531): "async fn a_batch_within_capacity_is_admitted(",
    ("polyoxide-polymarket/src/shared/signer_limit.rs", 540): "async fn adopting_a_higher_tier_admits_a_batch_that_was_impossible(",
    ("polyoxide-polymarket/src/shared/signer_limit.rs", 556): "async fn the_order_and_cancel_buckets_are_independent(",
    ("polyoxide-polymarket/src/shared/signer_limit.rs", 578): "async fn batch_cost_is_charged_in_full_not_as_one_request(",
    ("polyoxide-core/src/rate_limit.rs", 261): "let target = count.saturating_sub(count.div_ceil(RESERVED_FRACTION));",
    ("polyoxide-core/src/rate_limit.rs", 269): "const RESERVED_FRACTION: u32 = 10;",
    ("polyoxide-core/src/rate_limit.rs", 356): "fn a_published_150_per_10s_admits_135_per_window(",
    ("polyoxide-binance/tests/mock_usdm.rs", 409): "async fn a_418_is_not_retried_and_holds_the_next_request(",
    ("polyoxide-polymarket/src/clob/error.rs", 113): "let m = message.to_ascii_lowercase();",
    ("polyoxide-polymarket/src/clob/error.rs", 116): 'if m.contains("fak order") && (m.contains("no match")',
    ("polyoxide-polymarket/src/clob/error.rs", 156): "ApiError::Response(response) if response.status.as_u16() == 400 => {",
    ("polyoxide-polymarket/src/clob/error.rs", 384): "fn test_classify_recognizes_verbatim_venue_messages(",
    ("polyoxide-polymarket/src/clob/error.rs", 396): "fn test_classify_preserves_message_verbatim(",
    ("polyoxide-polymarket/src/clob/error.rs", 404): "fn test_classify_is_case_insensitive(",
    ("polyoxide-polymarket/src/clob/error.rs", 417): "fn test_classify_tolerates_curly_apostrophe_in_fok_message(",
    ("polyoxide-polymarket/src/clob/error.rs", 428): "fn test_classify_does_not_capture_neighbouring_400s(",
    ("polyoxide-polymarket/src/clob/error.rs", 459): "fn test_classify_requires_both_tokens(",
    ("polyoxide-polymarket/src/clob/error.rs", 498): "fn the_decode_splits_out_a_kill_only_on_a_400(",
    ("polyoxide-polymarket/src/clob/error.rs", 678): "fn the_decode_splits_out_a_heartbeat_mismatch_only_on_a_400(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 3530): "fn fak_unmatched_maps_to_typed_error_not_generic_validation(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 3564): "fn fok_unfilled_maps_to_typed_error_not_generic_validation(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 3628): "fn fak_prose_on_non_400_status_is_not_reclassified(",
    ("polyoxide-ws/src/backoff.rs", 84): "self.take().max(floor.min(self.max))",
    ("polyoxide-ws/src/backoff.rs", 207): "fn a_floor_extends_the_delay_and_never_shortens_it_drift_r11(",
    ("polyoxide-ws/src/backoff.rs", 223): "fn a_floor_past_the_ceiling_waits_the_ceiling_drift_r11(",
    ("polyoxide-ws/src/supervisor/tests.rs", 753):
        "async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11(",
    ("polyoxide-ws/src/supervisor/tests.rs", 775):
        "async fn a_retry_after_past_the_ceiling_waits_the_ceiling_drift_r11(",
    ("polyoxide-ws/src/supervisor/tests.rs", 795):
        "async fn a_start_in_an_outage_waits_out_the_first_refusals_retry_after_drift_r11(",
    ("polyoxide-polymarket/src/perps/ws/builder.rs", 730):
        "async fn a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11(",
    ("polyoxide-binance/tests/supervision_usdm_kit.rs", 437):
        "async fn a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11(",
    ("polyoxide-polymarket/tests/supervision_rtds.rs", 327):
        "async fn a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11(",
    ("polyoxide-polymarket/src/sports/supervised.rs", 484):
        "async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11(",
    ("polyoxide-binance/src/spot/api/market.rs", 167): ".route(Route::Depth { limit: Some(limit) })",
    ("polyoxide-binance/src/spot/ws/mod.rs", 85): "pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(500);",
    ("polyoxide-binance/src/spot/ws/mod.rs", 146): "fn the_limits_are_the_measured_ones(",
    ("polyoxide-binance/src/spot/ws/protocol.rs", 131): "fn the_stream_set_is_one_path_with_the_measured_limits(",
    ("polyoxide-binance/src/spot/ws/client.rs", 185): "fn the_documented_limits_are_the_measured_ones(",
    ("polyoxide-binance/src/spot/ws/client.rs", 232): "async fn requests_are_paced_at_500_ms(",
    ("polyoxide-binance/tests/supervision_spot.rs", 187): "async fn a_membership_change_waits_500_ms_for_the_one_before(",
    ("polyoxide-binance/tests/mock_spot.rs", 923): "fn every_market_builder_charges_its_route(",
    ("polyoxide-binance/tests/mock_spot.rs", 977): "async fn a_deep_depth_is_charged_its_band(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 225): "return Ok((exchange, \"3\"));",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 366): "let version = if is_v2_position_id(&order.token_id) {",
    ("polyoxide-polymarket/src/shared/protocol.rs", 52): "let reserved = U256::from(u64::MAX) << 40;",
    ("polyoxide-polymarket/src/shared/protocol.rs", 53): "(value & reserved) == U256::ZERO && decode_v2_outcome_position_id(id).is_some()",
    ("polyoxide-polymarket/src/clob/api/account.rs", 38): "\"CONDITIONAL-V2\"",
    ("polyoxide-polymarket/src/clob/client.rs", 489): "} else if is_v2_position_id(token_id) {",
    ("polyoxide-polymarket/src/gamma/types.rs", 356): "Some(ProtocolVersion::V1) => {",
    ("polyoxide-polymarket/src/gamma/types.rs", 363): "Some(ProtocolVersion::V2) => self",
    ("polyoxide-polymarket/src/relay/types.rs", 626): "if digits.len() == 64 && (digits.ends_with(\"00\") || digits.ends_with(\"01\")) {",
    ("polyoxide-polymarket/src/relay/deposit_wallet.rs", 274): "outcomeIndex: U256::from(position.outcome_index()),",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1706): "fn each_case_signs_at_the_version_and_exchange_py_sdk_chose(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1724): "fn the_domain_separator_and_struct_hash_match_py_sdk(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1804): "fn a_v2_deposit_wallet_session_key_reproduces_py_sdks_signature(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1789): "fn a_v2_deposit_wallet_owner_reproduces_py_sdks_signature(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1743): "fn the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1775): "fn plain_cases_reproduce_py_sdks_signatures(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 5156): "fn create_order_for_a_v2_id_reads_the_tick_size_never_neg_risk_and_signs_for_exchange_v3(",
    ("polyoxide-polymarket/tests/trading_clob.rs", 1943): "fn place_on_a_protocol_v2_key_names_the_order_by_its_exchange_v3_hash(",
    ("polyoxide-polymarket/src/shared/protocol.rs", 129): "fn the_rule_is_the_conjunction_of_py_sdks_two_answers(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1828): "fn a_v2_id_is_refused_where_exchange_v3_is_not_deployed(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1847): "fn signing_a_v2_id_on_amoy_is_refused_before_any_signature(",
    ("polyoxide-polymarket/src/shared/protocol.rs", 159): "fn the_four_fixture_ids_classify_as_the_venue_issues_them(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 5227): "fn a_v2_id_on_an_amoy_client_is_refused_at_signing_and_nothing_is_posted(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 5097): "fn balance_asset_types_follow_py_sdks_table(",
    ("polyoxide-polymarket/src/clob/account/mod.rs", 725): "fn sign_order_uses_the_deposit_wallet_target(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1221): "fn envelope_digest_matches_py_sdk_for_both_exchanges(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1542): "fn a_deposit_wallet_orders_hash_composes_py_sdks_domain_and_contents_hashes(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1578): "fn the_hash_follows_the_orders_exchange_and_refuses_an_unsupported_chain(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1562): "fn a_plain_orders_hash_is_the_digest_a_plain_signature_covers(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1295): "fn sign_order_as_owner_reproduces_py_sdk_signature(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1310): "fn sign_order_as_session_key_reproduces_py_sdk_signature(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 1374): "fn sign_order_as_uses_the_neg_risk_exchange_in_digest_and_trailer(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 825): "fn order_digest_differs_by_chain(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 811): "fn order_digest_uses_correct_exchange_for_neg_risk(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", 566): "fn v2_order_signature_is_deterministic(",
    ("polyoxide-polymarket/src/shared/protocol.rs", 170): "fn toy_ids_stay_v1_though_py_sdks_mask_alone_accepts_them(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 1873): "fn create_market_order_for_a_deposit_wallet_sets_maker_signer_and_type3(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 1735): "fn create_order_for_a_deposit_wallet_sets_maker_signer_and_type3(",
    ("polyoxide-polymarket/tests/mock_clob.rs", 1530): "fn place_order_signs_and_submits_v2_body_with_builder_code(",
    ("polyoxide-polymarket/tests/trading_clob.rs", 1293): "fn a_resting_post_only_order_is_open_and_carries_the_request(",
    ("polyoxide-polymarket/tests/trading_clob.rs", 1513): "fn the_tick_size_and_neg_risk_are_looked_up_only_when_the_options_lack_them(",
    ("polyoxide-polymarket/tests/outcome_ids_gamma.rs", 333): "fn the_field_the_version_names_must_be_present(",
    ("polyoxide-polymarket/tests/outcome_ids_gamma.rs", 299): "fn clob_token_ids_that_are_not_a_json_array_of_strings_are_malformed(",
    ("polyoxide-polymarket/tests/outcome_ids_gamma.rs", 317): "fn the_field_the_version_does_not_name_is_never_read(",
    ("polyoxide-polymarket/tests/outcome_ids_gamma.rs", 168): "fn the_version_picks_the_field_when_both_are_present(",
    ("polyoxide-polymarket/tests/outcome_ids_gamma.rs", 92): "fn the_selector_agrees_with_py_sdk_on_every_vector(",
    ("polyoxide-polymarket/tests/outcome_ids_gamma.rs", 138): "fn each_refused_vector_is_refused_for_its_reason(",
    ("polyoxide-polymarket/src/relay/deposit_wallet.rs", 626): "fn v2_condition_ids_narrow_as_py_sdk_does(",
    ("polyoxide-polymarket/src/relay/deposit_wallet.rs", 700): "fn router_redeem_calldata_matches_py_sdk_for_both_outcomes(",
    ("polyoxide-polymarket/src/relay/deposit_wallet.rs", 728): "fn the_router_redeem_batch_digest_matches_py_sdk(",
    ("polyoxide-polymarket/tests/mock_relay.rs", 2516): "fn v2_calls_target_the_router_with_py_sdks_calldata(",
    ("polyoxide-polymarket/tests/mock_relay.rs", 2630): "fn submit_v2_redeem_posts_the_router_call_for_a_proxy(",
    ("polyoxide-polymarket/tests/mock_relay.rs", 2669): "fn submit_v2_redeem_posts_the_router_call_for_a_deposit_wallet(",
    ("polyoxide-polymarket/tests/mock_relay.rs", 2583): "fn submit_v2_redeem_posts_the_router_call_for_a_safe(",
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
