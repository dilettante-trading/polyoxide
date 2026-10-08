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

## The rules

| Rule | Where it holds | Mutation | Tests that fail |
| --- | --- | --- | --- |
| (a) A 429 feeds the shared limiter before the retry decision, so a request with no retry left still publishes it | `polyoxide-core/src/request.rs:155`, `note_rate_limited`, before `should_retry` at `polyoxide-core/src/request.rs:157` (core's `Request` loop) | Move the `note_rate_limited` call into the `if let Some(backoff)` retry branch | `polyoxide-data/tests/mock_api.rs:1532` `a_429_makes_the_next_request_wait_even_though_it_never_saw_one` |
| (a), Binance's own loop | `polyoxide-binance/src/usdm/request.rs:136-149`, which holds the weight budget before deciding whether to retry | Replace the `match` with the cooldown arithmetic alone, and call `begin_cooldown` only inside `if retry.is_some()` | `polyoxide-binance/tests/mock_api.rs:626` `a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request`; `polyoxide-binance/tests/mock_api.rs:662` `a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute` |
| (b) `Retry-After` only extends a wait, never shortens it | `polyoxide-core/src/client.rs:154`, `requested.map_or(computed, \|r\| r.max(computed))` | `requested.unwrap_or(computed)` | `polyoxide-core/src/client.rs:509` `retry_after_below_our_own_backoff_does_not_shorten_the_wait` |
| (b), a zero `Retry-After` | `polyoxide-core/src/client.rs:149` (`*secs > 0.0`) and `polyoxide-core/src/client.rs:154` | `*secs >= 0.0` at the first and `requested.unwrap_or(computed)` at the second, which obeys Cloudflare's zero verbatim | `polyoxide-core/src/client.rs:509`; `polyoxide-core/src/client.rs:532` `retry_after_zero_still_backs_off_exponentially_across_attempts`; `polyoxide-data/tests/mock_api.rs:1509` `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop`; `polyoxide-data/tests/mock_api.rs:1532` |
| (c) Cooldowns only extend | `polyoxide-core/src/rate_limit.rs:363`, `begin_cooldown` keeps the later deadline | Assign `*slot = Some(until)` unconditionally | `polyoxide-core/src/rate_limit.rs:1907` `a_shorter_cooldown_never_cuts_a_longer_one_short` |
| (c), a cooldown extended mid-wait | `polyoxide-core/src/rate_limit.rs:381-396`, `await_cooldown` re-reads the deadline after each sleep | `return` after the `sleep_until` at `polyoxide-core/src/rate_limit.rs:394`, so it sleeps once | `polyoxide-core/src/rate_limit.rs:1925` `a_cooldown_extended_mid_wait_is_honoured_in_full` |
| (d) `quota()` leaves depth at one token, with no `allow_burst` | `polyoxide-core/src/rate_limit.rs:165-167` | Append `.allow_burst(NonZeroU32::new(count).unwrap())` | `polyoxide-core/src/rate_limit.rs:244` `no_quota_admits_more_than_its_published_count_in_one_window`; `polyoxide-core/src/rate_limit.rs:261` `every_quota_reserves_headroom_below_the_published_count`; `polyoxide-core/src/rate_limit.rs:284` `every_configured_bucket_satisfies_the_quota_it_publishes` |
| (e) `classify_order_kill` needs both the order kind and the kill token | `polyoxide-clob/src/error.rs:92` | `&&` → `\|\|` after `m.contains("fak order")` | `polyoxide-clob/src/error.rs:348` `test_classify_requires_both_tokens` |
| (e), case | `polyoxide-clob/src/error.rs:89`, the message is lowercased before matching | `let m = message.to_string();` | `polyoxide-clob/src/error.rs:273` `test_classify_recognizes_verbatim_venue_messages`; `polyoxide-clob/src/error.rs:285` `test_classify_preserves_message_verbatim`; `polyoxide-clob/src/error.rs:293` `test_classify_is_case_insensitive`; `polyoxide-clob/src/error.rs:306` `test_classify_tolerates_curly_apostrophe_in_fok_message`; `polyoxide-clob/tests/mock_api.rs:3365` `fak_unmatched_maps_to_typed_error_not_generic_validation`; `polyoxide-clob/tests/mock_api.rs:3399` `fok_unfilled_maps_to_typed_error_not_generic_validation` |
| (e), only a 400 is classified | `polyoxide-clob/src/error.rs:110-113`, `from_response` classifies `ApiError::Validation` alone | Add an `ApiError::Api { status, message }` arm that classifies `message` too | `polyoxide-clob/tests/mock_api.rs:3462` `fak_prose_on_non_400_status_is_not_reclassified` |

`polyoxide-clob/src/error.rs:317` `test_classify_does_not_capture_neighbouring_400s` fails
under none of the mutants above. It pins the 400s that neighbour the kill outcomes, so keep
it, but do not count it as holding any of these rules.

## Rule (a): sites not yet covered

The same call order also stands at five other sites, and no mutant is proved at any of them.
No tests are written for them, because Story 3.1 collapses them into core's one send loop
(AD-8), whose row is above. Until then, review an edit at one of these sites by hand.

- `polyoxide-core/src/client.rs:215`, `HttpClient::get_bytes`
- `polyoxide-clob/src/request.rs:262`, clob's request loop
- `polyoxide-relay/src/client.rs:293`, `polyoxide-relay/src/client.rs:392` and
  `polyoxide-relay/src/client.rs:1836`, relay's three loops
