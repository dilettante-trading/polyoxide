# Mutation-tested rules

Each rule below is held in place by tests, and each row proves it with a mutant: make the
change in the Mutation column, run the tests named, and every one of them fails. Restore
the line and they pass again. A test that only checks shape would pass under the mutant, so
a row whose mutant no longer fails its tests is a rule that nothing holds.

The list lives here rather than in [ARCHITECTURE.md](ARCHITECTURE.md), because that guide is
regenerated from the architecture spine (AD-21). When you move a line or a test named here,
update its row and prove the mutant again; `.github/scripts/tests/test_mutants_ledger.py`
fails when a cited line no longer holds the code it was cited for.

Every row was proved on 2026-10-08 at commit `a35a8ee`, on Rust 1.95 with `-j 4`, by
running `cargo test -p <crate> <target> -j 4 -- <test names>` with the mutant in place,
then without it.
Rows (e) and (e), case were proved again the same day on top of `8aac730`, after Stories
2.1 and 2.2 moved their `polyoxide-clob/src/error.rs` tests 41 lines down.
Row (a), Binance's own loop, was proved again on 2026-10-09 on top of `e35dcd2`, after
Stories 2.8 to 2.10 moved its `polyoxide-binance/tests/mock_api.rs` tests 10 lines up.
Rows (i) and (j) were proved on 2026-10-09 on top of `6779b93`, and row (c) again there,
after (j)'s test moved its lines 10 down.
Story 3.1 moved rule (a) into core's send loop and rule (b) into `RetryConfig::retry_delay`,
and added rows (a), the policy, (f) and (g). Those five were proved on 2026-10-09 on top of
`e05410c`, and rows (c), (d) and (j) again there, after the loop's imports moved their lines.

## The rules

| Rule | Where it holds | Mutation | Tests that fail |
| --- | --- | --- | --- |
| (a) A 429 holds the shared throttle before the retry decision, so a request with no retry left still publishes it | `polyoxide-core/src/send.rs:85`, the send loop's `hold` call, before the retry branch at `polyoxide-core/src/send.rs:89` | Call `hold` only in the retry branch, when the loop retries | `polyoxide-data/tests/mock_api.rs:1532` `a_429_makes_the_next_request_wait_even_though_it_never_saw_one`; `polyoxide-data/tests/mock_api.rs:1556` `a_429_on_the_pnl_host_holds_the_data_host`; `polyoxide-core/tests/send_loop.rs:295` `a_429_with_no_retry_left_still_holds` |
| (a), the policy | `polyoxide-core/src/hooks.rs:293`, `DefaultRetryPolicy`'s 429 arm holds whatever retries are left; `PolymarketRetryPolicy` defers to it for a 429 | `hold` only when `retries_left > 0` | `polyoxide-data/tests/mock_api.rs:1532`; `polyoxide-data/tests/mock_api.rs:1556`; `polyoxide-core/tests/send_loop.rs:295` |
| (a), Binance's own loop | `polyoxide-binance/src/usdm/request.rs:136-149`, which holds the weight budget before deciding whether to retry | Replace the `match` with the cooldown arithmetic alone, and call `begin_cooldown` only inside `if retry.is_some()` | `polyoxide-binance/tests/mock_api.rs:616` `a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request`; `polyoxide-binance/tests/mock_api.rs:652` `a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute` |
| (b) `Retry-After` only extends a wait, never shortens it | `polyoxide-core/src/rate_limit.rs:786`, `RetryConfig::retry_delay`'s `requested.map_or(computed, \|r\| r.max(computed))` | `requested.unwrap_or(computed)` | `polyoxide-core/src/client.rs:499` `retry_after_below_our_own_backoff_does_not_shorten_the_wait` |
| (b), a zero `Retry-After` | `polyoxide-core/src/rate_limit.rs:781` (`*secs > 0.0`) and `polyoxide-core/src/rate_limit.rs:786` | `*secs >= 0.0` at the first and `requested.unwrap_or(computed)` at the second, which obeys Cloudflare's zero verbatim | `polyoxide-core/src/client.rs:499`; `polyoxide-core/src/client.rs:522` `retry_after_zero_still_backs_off_exponentially_across_attempts`; `polyoxide-data/tests/mock_api.rs:1509` `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop`; `polyoxide-data/tests/mock_api.rs:1532`; `polyoxide-data/tests/mock_api.rs:1556` |
| (c) Cooldowns only extend | `polyoxide-core/src/rate_limit.rs:375`, `begin_cooldown` keeps the later deadline | Assign `*slot = Some(until)` unconditionally | `polyoxide-core/src/rate_limit.rs:1965` `a_shorter_cooldown_never_cuts_a_longer_one_short` |
| (c), a cooldown extended mid-wait | `polyoxide-core/src/rate_limit.rs:393-408`, `await_cooldown` re-reads the deadline after each sleep | `return` after the `sleep_until` at `polyoxide-core/src/rate_limit.rs:406`, so it sleeps once | `polyoxide-core/src/rate_limit.rs:1983` `a_cooldown_extended_mid_wait_is_honoured_in_full` |
| (d) `quota()` leaves depth at one token, with no `allow_burst` | `polyoxide-core/src/rate_limit.rs:167-169` | Append `.allow_burst(NonZeroU32::new(count).unwrap())` | `polyoxide-core/src/rate_limit.rs:246` `no_quota_admits_more_than_its_published_count_in_one_window`; `polyoxide-core/src/rate_limit.rs:263` `every_quota_reserves_headroom_below_the_published_count`; `polyoxide-core/src/rate_limit.rs:286` `every_configured_bucket_satisfies_the_quota_it_publishes` |
| (e) `classify_order_kill` needs both the order kind and the kill token | `polyoxide-clob/src/error.rs:92` | `&&` → `\|\|` after `m.contains("fak order")` | `polyoxide-clob/src/error.rs:389` `test_classify_requires_both_tokens` |
| (e), case | `polyoxide-clob/src/error.rs:89`, the message is lowercased before matching | `let m = message.to_string();` | `polyoxide-clob/src/error.rs:314` `test_classify_recognizes_verbatim_venue_messages`; `polyoxide-clob/src/error.rs:326` `test_classify_preserves_message_verbatim`; `polyoxide-clob/src/error.rs:334` `test_classify_is_case_insensitive`; `polyoxide-clob/src/error.rs:347` `test_classify_tolerates_curly_apostrophe_in_fok_message`; `polyoxide-clob/tests/mock_api.rs:3365` `fak_unmatched_maps_to_typed_error_not_generic_validation`; `polyoxide-clob/tests/mock_api.rs:3399` `fok_unfilled_maps_to_typed_error_not_generic_validation` |
| (e), only a 400 is classified | `polyoxide-clob/src/error.rs:110-113`, `from_response` classifies `ApiError::Validation` alone | Add an `ApiError::Api { status, message }` arm that classifies `message` too | `polyoxide-clob/tests/mock_api.rs:3462` `fak_prose_on_non_400_status_is_not_reclassified` |
| (f) `observe` runs on every response, the last attempt included | `polyoxide-core/src/send.rs:78`, before the policy decides | Call `observe` only when `retries_left > 0` | `polyoxide-core/tests/send_loop.rs:248` `observe_sees_the_last_attempt` |
| (g) A retry sleeps at least the loop's floor, whatever wait the policy returns | `polyoxide-core/src/send.rs:91`, `floor.max(wait)` | Sleep `wait` | `polyoxide-core/tests/send_loop.rs:274` `a_zero_wait_still_sleeps_the_floor`; `polyoxide-core/tests/send_loop.rs:318` `the_429_hold_is_retry_delay_zero_not_the_attempts_wait` |
| (i) The per-signer layer keeps `allow_burst`, so a signer bucket holds its published burst | `polyoxide-core/src/signer_limit.rs:272`, `.allow_burst(..)` on each bucket's quota | Delete the `.allow_burst(..)` call, leaving governor's capacity of one token | `polyoxide-core/src/signer_limit.rs:470` `a_batch_within_capacity_is_admitted`; `polyoxide-core/src/signer_limit.rs:479` `adopting_a_higher_tier_admits_a_batch_that_was_impossible`; `polyoxide-core/src/signer_limit.rs:495` `the_order_and_cancel_buckets_are_independent`; `polyoxide-core/src/signer_limit.rs:517` `batch_cost_is_charged_in_full_not_as_one_request` |
| (j) A window quota aims at 90% of its published count, the reserve measured on `/closed-positions` | `polyoxide-core/src/rate_limit.rs:188`, `RESERVED_FRACTION`, and `polyoxide-core/src/rate_limit.rs:180`, where `sustained_slots` takes the reserve off | `RESERVED_FRACTION = 20` at the first, which fails only the first test; `let target = count;` at the second | `polyoxide-core/src/rate_limit.rs:310` `a_published_150_per_10s_admits_135_per_window`; `polyoxide-core/src/rate_limit.rs:263` `every_quota_reserves_headroom_below_the_published_count` (the second mutant only) |

`polyoxide-clob/src/error.rs:358` `test_classify_does_not_capture_neighbouring_400s` fails
under none of the mutants above. It pins the 400s that neighbour the kill outcomes, so keep
it, but do not count it as holding any of these rules.

Row (g) is held by the two loop tests alone. Under its mutant
`polyoxide-data/tests/mock_api.rs:1509` fails only when both of its holds jitter short (once
in three runs), and `polyoxide-perps/tests/mock_api.rs:544`
`a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff` never does: a 429's
hold, `retry_delay(0)`, already keeps the next attempt back for as long as the floor would,
and on perps the route's own bucket does too. Neither counts as holding (g).

## Rule (a): sites not yet covered

The same call order, `note_rate_limited` before `should_retry`, also stands at four
hand-written loops, and no mutant is proved at any of them. No tests are written for them,
because Stories 3.4 and 3.5 move them onto core's one send loop (AD-8), whose rows are above.
Until then, review an edit at one of these sites by hand.

- `polyoxide-clob/src/request.rs:262`, clob's request loop
- `polyoxide-relay/src/client.rs:293`, `polyoxide-relay/src/client.rs:392` and
  `polyoxide-relay/src/client.rs:1836`, relay's three loops
