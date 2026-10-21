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
H3's `decode_json` then moved the loop's lines 1 down, Binance's 6 up and clob's 7 up; rows
(a), (a) for Binance, (f) and (g) were proved again on top of `0461462`.
DRIFT R10's hold warning moved the loop's lines 5 down and its tests 4 down; rows (a),
(a), the policy, (f) and (g) were proved again on top of `50eca25`.
Story 3.2's `WindowQuotaTable` moved `polyoxide-core/src/rate_limit.rs` about; rows (b), (c),
(d) and (j) were proved again on top of `d425a23`.
Story 3.2's public-API rewrite of the table tests moved rows (b) and (c) and rewrote (d)'s
sweep onto `RateLimiter::rows()`; rows (b), (c) and (d) were proved again on top of `13c00ed`.
Its location-only move took the table, agreement and cooldown tests to
`polyoxide-core/tests/polymarket_limits.rs`; rows (b), (c), (d) and (j) were proved again on
top of `ed0741d`.
Story 3.3 extracted the cooldown into `Hold` (`polyoxide-core/src/hold.rs`); rows (b), (c), (d)
and (j) were proved again on top of `14c23f1`: (c) with the new `a_hold_never_shortens`, (c), a
hold extended mid-wait, with `Hold`'s own test, and the new row for the re-check.
Its `CapacityBucket` replaced the signer layer's governor buckets; row (i) was re-cited at the
bucket's capacity argument and proved again on top of `dc1127a`, its four tests unchanged.
`polymarket::ClobThrottle` moved the signer layer's lines; row (i) was proved again on top of
`34d60d3`.
Bundle F's review patches moved `rate_limit.rs`'s lines 3 and 26 down and cited the re-check
from its `// AD-23:` line, since its `wait` line's text also stands earlier in `acquire`; rows
(b), (d), (j) and the re-check were proved again on top of `ed3fd2d`.
Row (k), Binance's 418 hold, was proved on 2026-10-09 on top of `5f93b60`.
DRIFT R4 put every `Retry-After` read through `polyoxide_venue::parse_retry_after`; rows (b) and
(b), a zero were re-cited there and proved again on top of `3aeddc2`.
Bundle G's core additions (`RequestParts::timeout`, `RetryConfig::attempt_info`, `Hold::is_held`,
and `PolymarketRetryPolicy`'s `Fail` with no retry left) moved the loop's lines 3 up, the
policy's hold 4 down and `Hold`'s tests 5 down; rows (a), (a), the policy, (f), (g) and (c)
were proved again on top of `a0d2771`.
Story 3.4 moved clob onto the send loop; its error imports moved rows (e) 3 lines down and the
hand-written `From<ApiError>` moved their tests 61 down. Rows (e), (e), case and (e), only a
400 were proved again on top of `d8cc425`.
Story 3.5 moved relay's three loops, the last sites rule (a) did not cover, onto the send loop,
whose rows hold them.
Story 3.6 moved Binance onto the send loop: row (a), Binance's own loop, became (a), Binance's
policy, and row (k) moved to the policy's 418 arm. Both were proved on top of `207c70f`.
The transitional `HttpClient` methods' tests were rewritten onto `RetryConfig::retry_delay` and
`PolymarketRetryPolicy::decide`, keeping their names and answers; rows (b) and (b), a zero were
proved again at the rewritten tests on top of `590a93f`.
Removing `should_retry`, `note_rate_limited` and `acquire_rate_limit` moved those tests 63 lines
up; rows (b) and (b), a zero were proved again on top of `6180817`.
Bundle G's review documented the retry `WARN`'s wait in `send.rs`, moving rows (a), (f) and (g)
three lines down; all three were proved again on top of `acadd99`.
Story 3.11 made clob's `From<ApiError>` its one decode, which reads an `ApiError::Response`'s
status where `from_response` matched `ApiError::Validation`, and moved the kill classifier 7
lines down and its tests 29 up; row (e), only a 400 was re-written for the new arm, and rows
(e), (e), case and (e), only a 400 were proved on 2026-10-10 on top of `718a052`.
Row (b), a socket reconnect, DRIFT R11's copy of rule (b) in `polyoxide-ws`, was proved on
2026-10-12 on top of `f35a15f`, by running
`cargo test -p <crate> --all-features -j 4 -- drift_r11` for `polyoxide-ws`, perps, Binance,
rtds and sports with the mutant in place, then without it.
Story 5.2 moved Polymarket's code and the tests that run on it out of core into
`polyoxide-polymarket`: `tests/send_loop.rs` and `tests/polymarket_limits.rs` whole, the
signer layer with its inline tests to `src/shared/signer_limit.rs` (its lines 2 down), and
core's `client.rs` retry-policy tests to `src/shared/policy.rs`, which moved row (b)'s tests
97 lines up. The rule lines in core did not move. Rows (a), (a), the policy, (b), (b), a
zero, (c), (d), (f), (g) and (i) were proved again on 2026-10-12 on top of `011c3bf`, by
running each row's tests with its mutant in place, then without it, the moved ones through
`cargo test -p polyoxide-polymarket --all-features`.
Story 5.3 folded `polyoxide-data` into `polyoxide-polymarket`'s `data` module, so its
`tests/mock_api.rs` is `polyoxide-polymarket/tests/mock_data.rs`, where rustfmt re-wrapped two
earlier assertions under their longer paths, moving the tests of rows "(a)", "(a), the policy"
and "(b), a zero" 6 lines down; and repointing clob at the `gamma` module re-wrapped one of its
`error.rs` tests, moving the tests of row "(e)" 2 lines down. The rule lines did not move. Rows
"(a)", "(a), the policy", "(b), a zero", "(e)", "(e), case" and "(e), only a 400" were proved
again on 2026-10-13 on top of `7d08b41`, by running each row's tests with its mutant in place,
then without it.
Story 5.4 folded `polyoxide-clob` into `polyoxide-polymarket`'s `clob` module, so its
`src/error.rs` is `polyoxide-polymarket/src/clob/error.rs`, where rustfmt put an import and a
test's match pattern each on one line under the shorter `crate::` paths, moving the rule lines of
rows "(e)", "(e), case" and "(e), only a 400" 2 lines up and their tests 4 up; and its
`tests/mock_api.rs` is `polyoxide-polymarket/tests/mock_clob.rs`, where rustfmt re-wrapped earlier
calls under the longer `polyoxide_polymarket::clob` paths, moving those rows' tests 21 lines down.
The three rows were proved again on 2026-10-13 on top of `a680a2e`, by running each row's tests
with its mutant in place, then without it, through `cargo test -p polyoxide-polymarket`.
Story 5.5 folded `polyoxide-perps`, `polyoxide-rtds` and `polyoxide-sports` into
`polyoxide-polymarket`'s `perps`, `rtds` and `sports` modules. Perps' `src/ws/builder.rs` is
`polyoxide-polymarket/src/perps/ws/builder.rs` and its `tests/mock_api.rs` is
`tests/mock_perps.rs`, both at the same lines; sports' `src/supervised.rs` is
`src/sports/supervised.rs`, at the same lines; and rtds' `tests/supervision.rs` is
`tests/supervision_rtds.rs`, where rustfmt re-wrapped earlier frames under the longer
`polyoxide_polymarket::rtds` paths, moving row "(b), a socket reconnect"'s test 12 lines down.
The rule line did not move. That row was proved again on 2026-10-14 on top of `f217bb3`, by
running `cargo test -p <crate> --all-features -j 4 --no-fail-fast -- drift_r11` for
`polyoxide-ws`, Binance and `polyoxide-polymarket` with its mutant in place, then without it;
and row (g)'s note on `mock_perps.rs` was checked again there under (g)'s mutant.
Story 5.6 gave `polyoxide-binance`'s suites the `usdm` module's name: its `tests/mock_api.rs` is
`tests/mock_usdm.rs` and its `tests/supervision_kit.rs` is `tests/supervision_usdm_kit.rs`, both
at the same lines, and `src/usdm/policy.rs` kept its lines while its imports moved under
`crate::usdm`. Rows "(a), Binance's policy", "(b), a socket reconnect" and "(k)" were proved
again on 2026-10-14 on top of `50a8ea5`, by running each row's tests with its mutant in place,
then without it, through `cargo test -p polyoxide-binance --all-features`, and for row
"(b), a socket reconnect" `cargo test -p <crate> --all-features -j 4 --no-fail-fast -- drift_r11`
for `polyoxide-ws`, Binance and `polyoxide-polymarket`.
Stories 5.11 and 5.12 moved no cited line or test. All twenty mutants, one per row and two
each for rows (j) and (k), were proved again on 2026-10-15 on top of `e699e69`, the S2 hub's
final tree, on Rust 1.95 with `-j 4`: each row's tests passed, then failed with its mutant in
place, then passed again once the file was restored, with `git diff --exit-code` clean. They
ran through `cargo test -p <crate> --all-features`, with `--lib` for a crate's inline tests,
and for row "(b), a socket reconnect" `cargo test -p <crate> --all-features -- drift_r11` for
`polyoxide-ws`, Binance and `polyoxide-polymarket`, where perps' and rtds' tests of one name
both failed.
Story 6.5's `ClobError::Data`, a new variant with its three `Classify` arms, moved the rule
lines of rows "(e)", "(e), case" and "(e), only a 400" 6 lines down and their `error.rs` tests
12 down, with the note on `test_classify_does_not_capture_neighbouring_400s`; their
`mock_clob.rs` tests did not move. The three rows were proved again on 2026-10-16 on top of
`875f214`, by running each row's tests with its mutant in place, then without it, through
`cargo test -p polyoxide-polymarket --all-features --no-fail-fast --lib --test mock_clob`.
The fix for #70 (`46aa0f4`) added `mock_clob.rs` tests above the three rows' `mock_clob.rs`
tests, moving them 143 lines down; their rule lines and `error.rs` tests did not move. Rows
"(e), case" and "(e), only a 400" were proved again on 2026-10-21 on top of `7a03b60`, on Rust
1.99.0 with `-j 4`: each row's tests passed, then failed with its mutant in place, then passed
again once the file was restored, with `git diff --exit-code` clean, through the same command.

## The rules

| Rule | Where it holds | Mutation | Tests that fail |
| --- | --- | --- | --- |
| (a) A 429 holds the shared throttle before the retry decision, so a request with no retry left still publishes it | `polyoxide-core/src/send.rs:91`, the send loop's `hold` call, before the retry branch at `polyoxide-core/src/send.rs:95` | Call `hold` only in the retry branch, when the loop retries | `polyoxide-polymarket/tests/mock_data.rs:1538` `a_429_makes_the_next_request_wait_even_though_it_never_saw_one`; `polyoxide-polymarket/tests/mock_data.rs:1562` `a_429_on_the_pnl_host_holds_the_data_host`; `polyoxide-polymarket/tests/send_loop.rs:299` `a_429_with_no_retry_left_still_holds` |
| (a), the policy | `polyoxide-core/src/hooks.rs:297`, `DefaultRetryPolicy`'s 429 arm holds whatever retries are left; `PolymarketRetryPolicy` defers to it for a 429, and keeps the hold when it turns the retry into a `Fail` with no retry left | `hold` only when `retries_left > 0` | `polyoxide-polymarket/tests/mock_data.rs:1538`; `polyoxide-polymarket/tests/mock_data.rs:1562`; `polyoxide-polymarket/tests/send_loop.rs:299`; `polyoxide-polymarket/tests/send_loop.rs:580` `a_429_with_no_retry_left_is_fail_with_its_hold` |
| (a), Binance's policy | `polyoxide-binance/src/usdm/policy.rs:53`, `UsdmRetryPolicy`'s arm for a 429 with no retry left holds the weight budget for its `Retry-After`, or to the next UTC minute | `hold: None` in that arm, so a 429 holds only when a retry is left | `polyoxide-binance/tests/mock_usdm.rs:616` `a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request`; `polyoxide-binance/tests/mock_usdm.rs:652` `a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute` |
| (b) `Retry-After` only extends a wait, never shortens it | `polyoxide-venue/src/retry_after.rs:45`, `retry_delay`'s `requested.map_or(computed, \|requested\| requested.max(computed))`, which `RetryConfig::retry_delay` calls | `requested.unwrap_or(computed)` | `polyoxide-core/src/client.rs:319` `retry_after_below_our_own_backoff_does_not_shorten_the_wait`; `polyoxide-venue/src/retry_after.rs:103` `a_requested_wait_only_ever_lengthens_the_backoff` |
| (b), a zero `Retry-After` | `polyoxide-venue/src/retry_after.rs:29` (`secs <= 0.0`), `polyoxide-venue/src/retry_after.rs:35` (the zero filter) and `polyoxide-venue/src/retry_after.rs:45`: the one parser, since DRIFT R4, refuses a zero twice | `secs < 0.0` at the first, `Some(wait)` unfiltered at the second and `requested.unwrap_or(computed)` at the third, which together obey Cloudflare's zero verbatim; the first two alone leave the tests passing, and the third alone is row (b)'s mutant | `polyoxide-core/src/client.rs:319`; `polyoxide-core/src/client.rs:338` `retry_after_zero_still_backs_off_exponentially_across_attempts`; `polyoxide-polymarket/tests/mock_data.rs:1515` `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop`; `polyoxide-polymarket/tests/mock_data.rs:1538`; `polyoxide-polymarket/tests/mock_data.rs:1562`; `polyoxide-venue/src/retry_after.rs:55` `every_parser_row` |
| (b), a socket reconnect (DRIFT R11) | `polyoxide-ws/src/backoff.rs:84`, `Backoff::take_at_least`'s `self.take().max(floor.min(self.max))`: a refusal's `Retry-After` only extends the reconnect delay, up to the ceiling. The kit's `Supervisor` calls it with `Protocol::retry_after`, and rtds and sports at their own reconnect waits | `self.take()`, the floor ignored | `polyoxide-ws/src/backoff.rs:207` `a_floor_extends_the_delay_and_never_shortens_it_drift_r11`; `polyoxide-ws/src/backoff.rs:223` `a_floor_past_the_ceiling_waits_the_ceiling_drift_r11`; `polyoxide-ws/src/supervisor/tests.rs:753` `a_reconnect_waits_out_a_refusals_retry_after_drift_r11`; `polyoxide-ws/src/supervisor/tests.rs:775` `a_retry_after_past_the_ceiling_waits_the_ceiling_drift_r11`; `polyoxide-ws/src/supervisor/tests.rs:795` `a_start_in_an_outage_waits_out_the_first_refusals_retry_after_drift_r11`; `polyoxide-polymarket/src/perps/ws/builder.rs:710` `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`; `polyoxide-binance/tests/supervision_usdm_kit.rs:437` `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`; `polyoxide-polymarket/tests/supervision_rtds.rs:327` `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`; `polyoxide-polymarket/src/sports/supervised.rs:419` `a_reconnect_waits_out_a_refusals_retry_after_drift_r11` |
| (c) Cooldowns only extend | `polyoxide-core/src/hold.rs:72`, `Hold::extend` keeps the later deadline | Assign `*slot = Some(until)` unconditionally | `polyoxide-polymarket/tests/polymarket_limits.rs:1036` `a_shorter_cooldown_never_cuts_a_longer_one_short`; `polyoxide-core/src/hold.rs:135` `a_hold_never_shortens` |
| (c), a hold extended mid-wait | `polyoxide-core/src/hold.rs:78-93`, `Hold::wait` re-reads the deadline after each sleep | `return` after the `sleep_until` at `polyoxide-core/src/hold.rs:91`, so it sleeps once | `polyoxide-core/src/hold.rs:150` `a_hold_extended_mid_wait_is_honoured_in_full` |
| (c), the re-check after a bucket wait (AD-23) | `polyoxide-core/src/rate_limit.rs:577-579`, `RateLimiter::acquire` waits out the hold again after its buckets | Delete that second `wait` | `polyoxide-core/src/rate_limit.rs:830` `a_hold_set_during_a_bucket_wait_is_honoured` |
| (d) `quota()` leaves depth at one token, with no `allow_burst` | `polyoxide-core/src/rate_limit.rs:183-186` | Append `.allow_burst(NonZeroU32::new(count).unwrap())` | `polyoxide-core/src/rate_limit.rs:263` `no_quota_admits_more_than_its_published_count_in_one_window`; `polyoxide-core/src/rate_limit.rs:280` `every_quota_reserves_headroom_below_the_published_count`; `polyoxide-polymarket/tests/polymarket_limits.rs:21` `every_configured_bucket_satisfies_the_quota_it_publishes` |
| (e) `classify_order_kill` needs both the order kind and the kill token | `polyoxide-polymarket/src/clob/error.rs:106` | `&&` → `\|\|` after `m.contains("fak order")` | `polyoxide-polymarket/src/clob/error.rs:431` `test_classify_requires_both_tokens` |
| (e), case | `polyoxide-polymarket/src/clob/error.rs:103`, the message is lowercased before matching | `let m = message.to_string();` | `polyoxide-polymarket/src/clob/error.rs:356` `test_classify_recognizes_verbatim_venue_messages`; `polyoxide-polymarket/src/clob/error.rs:368` `test_classify_preserves_message_verbatim`; `polyoxide-polymarket/src/clob/error.rs:376` `test_classify_is_case_insensitive`; `polyoxide-polymarket/src/clob/error.rs:389` `test_classify_tolerates_curly_apostrophe_in_fok_message`; `polyoxide-polymarket/tests/mock_clob.rs:3530` `fak_unmatched_maps_to_typed_error_not_generic_validation`; `polyoxide-polymarket/tests/mock_clob.rs:3564` `fok_unfilled_maps_to_typed_error_not_generic_validation` |
| (e), only a 400 is classified | `polyoxide-polymarket/src/clob/error.rs:133`, clob's decode, `From<ApiError>`, classifies an `ApiError::Response` only when its status is 400 | Drop the arm's `if response.status.as_u16() == 400` guard, so every response's message is classified | `polyoxide-polymarket/tests/mock_clob.rs:3628` `fak_prose_on_non_400_status_is_not_reclassified`; `polyoxide-polymarket/src/clob/error.rs:470` `the_decode_splits_out_a_kill_only_on_a_400` |
| (f) `observe` runs on every response, the last attempt included | `polyoxide-core/src/send.rs:84`, before the policy decides | Call `observe` only when `retries_left > 0` | `polyoxide-polymarket/tests/send_loop.rs:252` `observe_sees_the_last_attempt` |
| (g) A retry sleeps at least the loop's floor, whatever wait the policy returns | `polyoxide-core/src/send.rs:97`, `floor.max(wait)` | Sleep `wait` | `polyoxide-polymarket/tests/send_loop.rs:278` `a_zero_wait_still_sleeps_the_floor`; `polyoxide-polymarket/tests/send_loop.rs:322` `the_429_hold_is_retry_delay_zero_not_the_attempts_wait` |
| (i) The per-signer layer's buckets hold their published burst, as governor's `allow_burst` did before Story 3.3 | `polyoxide-polymarket/src/shared/signer_limit.rs:278`, where each signer `CapacityBucket` takes the tier's burst as its capacity | `let capacity = 1;`, a bucket of one token | `polyoxide-polymarket/src/shared/signer_limit.rs:525` `a_batch_within_capacity_is_admitted`; `polyoxide-polymarket/src/shared/signer_limit.rs:534` `adopting_a_higher_tier_admits_a_batch_that_was_impossible`; `polyoxide-polymarket/src/shared/signer_limit.rs:550` `the_order_and_cancel_buckets_are_independent`; `polyoxide-polymarket/src/shared/signer_limit.rs:572` `batch_cost_is_charged_in_full_not_as_one_request` |
| (j) A window quota aims at 90% of its published count, the reserve measured on `/closed-positions` | `polyoxide-core/src/rate_limit.rs:216`, `RESERVED_FRACTION`, and `polyoxide-core/src/rate_limit.rs:208`, where `sustained_slots` takes the reserve off | `RESERVED_FRACTION = 20` at the first, which fails only the first test; `let target = count;` at the second | `polyoxide-core/src/rate_limit.rs:303` `a_published_150_per_10s_admits_135_per_window`; `polyoxide-core/src/rate_limit.rs:280` `every_quota_reserves_headroom_below_the_published_count` (the second mutant only) |
| (k) A `418` holds every request on the weight budget, for its `Retry-After` or two minutes, so no request is sent into a ban | `polyoxide-binance/src/usdm/policy.rs:57`, `UsdmRetryPolicy`'s 418 arm | `hold: None`, which fails the first two tests; `asked.unwrap_or(Duration::ZERO)`, which fails the third | `polyoxide-binance/tests/mock_usdm.rs:409` `a_418_is_not_retried_and_holds_the_next_request`; `polyoxide-binance/tests/mock_usdm.rs:707` `every_usdm_on_one_budget_is_held_by_one_ban`; `polyoxide-binance/src/usdm/policy.rs:185` `a_418_without_retry_after_holds_two_minutes` |

`polyoxide-polymarket/tests/polymarket_limits.rs:1054` `a_cooldown_extended_mid_wait_is_honoured_in_full`
held row (c), a hold extended mid-wait, until Story 3.3. It drives `RateLimiter::acquire`, whose
re-check after its buckets now also waits out an extended hold, so it passes under that row's
mutant; `Hold`'s own test holds the row, and the re-check has a row of its own.

`polyoxide-polymarket/src/clob/error.rs:400` `test_classify_does_not_capture_neighbouring_400s` fails
under none of the mutants above. It pins the 400s that neighbour the kill outcomes, so keep
it, but do not count it as holding any of these rules.

Row (g) is held by the two loop tests alone. Under its mutant
`polyoxide-polymarket/tests/mock_data.rs:1515` fails only when both of its holds jitter short (once
in three runs), and `polyoxide-polymarket/tests/mock_perps.rs:544`
`a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff` never does: a 429's
hold, `retry_delay(0)`, already keeps the next attempt back for as long as the floor would,
and on perps the route's own bucket does too. Neither counts as holding (g).
