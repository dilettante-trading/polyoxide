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
The adoption of drift issue #73 (`3691eda`) added `ClobError::HeartbeatIdMismatch` with its three
`Classify` arms and the decode's `heartbeat_mismatch`, moving the rule lines of rows "(e)" and
"(e), case" 10 lines down and of "(e), only a 400" 23 down, and their `error.rs` tests 28 down,
with the note on `test_classify_does_not_capture_neighbouring_400s`; their `mock_clob.rs` tests
did not move. Its `the_decode_splits_out_a_heartbeat_mismatch_only_on_a_400` fails under row
"(e), only a 400"'s mutant too, so the row names it. The three rows were proved again on
2026-10-23 on top of `3691eda`, on Rust 1.99.1 with `-j 4`: each row's tests passed, then failed
with its mutant in place, then passed again once the file was restored, with
`git diff --exit-code` clean, through the same command.
The Binance spot plan's Story 2 moved `UsdmRetryPolicy`'s decision into
`polyoxide-binance/src/shared/policy.rs` (`shared::policy::decide`), which every Binance product's
policy calls, and the 418 test's line in `src/usdm/policy.rs` moved with the file's top. Rows "(a),
Binance's policy" and "(k)" were re-cited there and proved again on 2026-11-02 on the tree committed
as `ec304f56`, on Rust 1.99.0: each row's tests passed, then failed with its mutant in place (two
for row (k)), then passed again once the file was restored, through `cargo test -p polyoxide-binance
--all-features`.

Row (l), spot's depth re-pricing, was proved on 2026-11-02 on top of `7bf6f943`, on Rust 1.99.0 with
`-j 4`, after Story 5's review round collapsed `mock_spot.rs`'s `market_*` helpers onto the suite's
own and moved the two tests up: the two tests passed, then failed with the mutant in place, then
passed again once the line was restored, with `git diff --exit-code` clean, through the command in
Story 5 of the Binance spot plan (`cargo test -p polyoxide-binance --no-default-features --features
spot --test mock_spot -j 4 -- every_market_builder_charges_its_route
a_deep_depth_is_charged_its_band`).

Row (m), spot's socket pace, was proved on 2026-11-02 on top of `57bb70c5`, on Rust 1.99.0 with `-j 4`: its five tests passed, then failed with the mutant in place, then passed again once the line was restored, with `git diff --exit-code` clean, through the commands in Story 4 of the Binance spot streams plan (`cargo test -p polyoxide-binance --no-default-features --features spot-ws,test-server -j 4 --lib -- the_limits_are_the_measured_ones the_stream_set_is_one_path_with_the_measured_limits the_documented_limits_are_the_measured_ones requests_are_paced_at_500_ms` and `--test supervision_spot -- a_membership_change_waits_500_ms_for_the_one_before`).

The merge of `main` into the wasm32 branch (`50111953`) moved Binance spot onto the runtime and
transport seams, and its follow-up added two lines to `SpotWs::ping`'s docs, which moved row (m)'s
`client.rs` tests from lines 183 and 230 to 185 and 232. The row was proved again on 2026-11-03 on
top of `70e5c55d` (the landing merge, which holds the follow-up `eea6eaf8` and so the cited lines),
on Rust 1.99.0 with `-j 4`, through the same two commands: its five tests
passed, then failed with `Duration::from_millis(250)` in place, then passed again once the line was
restored, with `git diff --exit-code` clean. Rows (a), (k) and (l) did not move in the merge: the
`shared/` port changed an import's path on a line of its own, not the lines they cite.

The wasm32 plan's story 2 put `polyoxide-core`'s holds, capacity buckets and window quotas, and the clients that own them, on an injected runtime, which moved lines of `client.rs`, `hold.rs`, `rate_limit.rs` and `polyoxide-polymarket/src/shared/signer_limit.rs` and changed `Hold::wait`'s sleep; rows (b), (b), a zero `Retry-After`, (c), (c), a hold extended mid-wait, (c), the re-check after a bucket wait, (d), (i) and (j) were proved again on top of `02dc91c4`, on Rust 1.99.0 with `-j 4`: each row's tests failed with its mutant in place and passed again once the file was restored.

Story 2's review fix (`e0220803`) added 14 lines above row (c) in `polyoxide-core/src/rate_limit.rs`, moving its citations to `657-659` and its test to `910`; the row's mutant (delete the second `self.inner.hold.wait().await`) was proved again on `57bdbf66`, on Rust 1.99.0: `a_hold_set_during_a_bucket_wait_is_honoured` failed with the line deleted and passed again once the file was restored.

Story 4 of the wasm plan (`HttpTransport`) added `HttpClient::transport`, the config's transport setter and the transport tests to `polyoxide-core/src/client.rs`, which moved row (b)'s test `retry_after_below_our_own_backoff_does_not_shorten_the_wait` and the zero-`Retry-After` row's `retry_after_zero_still_backs_off_exponentially_across_attempts` 9 lines down, to `client.rs:353` and `client.rs:372`; the two rows were proved again on 2026-11-02 on top of `682b9921`, on Rust 1.99.0 with `-j 4`: row (b)'s tests failed with `requested.unwrap_or(computed)` in place, the zero row's six tests failed with its three mutants together, and all of them passed again once `polyoxide-venue/src/retry_after.rs` was restored (`git diff --exit-code` clean). The send loop's four cited lines (rows (a), (f) and (g)) did not move.

The wasm32 plan's story 8 gave the sports feed a rule for a transport that hides the server's pings, which moved `polyoxide-polymarket/src/sports/supervised.rs`'s test of row (b), a socket reconnect; that row was proved again on top of `0c36e704`, on Rust 1.99.0 with `-j 4`: the test failed with the mutant in place and passed again once the file was restored. The story's sports review fixes resolved the default transport once in `connect`, which moved that test from line 475 to 468; the row was proved again on top of that change, with `Backoff::take_at_least` reduced to `self.take()`: the test failed with the mutant in place and passed again once `backoff.rs` was restored.

The owner's follow-up (2026-11-03) made the sports feed arm no timer with its staleness check off, which added 13 lines above row (b)'s sports test and moved it from line 468 to 481. The row was proved again on top of that change, on Rust 1.99.0 with `-j 4`, with `Backoff::take_at_least` reduced to `self.take()`: `a_reconnect_waits_out_a_refusals_retry_after_drift_r11` failed with the mutant in place and passed again once `backoff.rs` was restored.

The follow-up review's fix for the `stale_after` setter's docs (M3) added 3 lines above it, moving it from line 481 to 484; the row was proved again with the same mutant, on Rust 1.99.0 with `-j 4`: `a_reconnect_waits_out_a_refusals_retry_after_drift_r11` failed with the mutant in place and passed again once `backoff.rs` was restored.

The Protocol V2 trading work (#51) added rows (n) to (x). Each was proved on 2026-11-04 on top of `00442584`, on rustc 1.99.0 with `CARGO_INCREMENTAL=0`, by running `cargo test -p polyoxide-polymarket --all-features --no-fail-fast` over the test targets of its mutation (`--lib` with `--test mock_clob --test trading_clob` for the CLOB rows, with `--test outcome_ids_gamma --test venue_clob` for (u), with `--test mock_relay` for (v) and (w)): its tests passed, then failed with the mutant in place, then passed again once the file was restored, with `git diff --exit-code` clean. Row (x) is the control: its mutant swaps two pure conditions and every test passed with it in place, which shows the battery fails a mutant by its behaviour and not by the edit.

## The rules

| Rule | Where it holds | Mutation | Tests that fail |
| --- | --- | --- | --- |
| (a) A 429 holds the shared throttle before the retry decision, so a request with no retry left still publishes it | `polyoxide-core/src/send.rs:91`, the send loop's `hold` call, before the retry branch at `polyoxide-core/src/send.rs:95` | Call `hold` only in the retry branch, when the loop retries | `polyoxide-polymarket/tests/mock_data.rs:1538` `a_429_makes_the_next_request_wait_even_though_it_never_saw_one`; `polyoxide-polymarket/tests/mock_data.rs:1562` `a_429_on_the_pnl_host_holds_the_data_host`; `polyoxide-polymarket/tests/send_loop.rs:299` `a_429_with_no_retry_left_still_holds` |
| (a), the policy | `polyoxide-core/src/hooks.rs:297`, `DefaultRetryPolicy`'s 429 arm holds whatever retries are left; `PolymarketRetryPolicy` defers to it for a 429, and keeps the hold when it turns the retry into a `Fail` with no retry left | `hold` only when `retries_left > 0` | `polyoxide-polymarket/tests/mock_data.rs:1538`; `polyoxide-polymarket/tests/mock_data.rs:1562`; `polyoxide-polymarket/tests/send_loop.rs:299`; `polyoxide-polymarket/tests/send_loop.rs:580` `a_429_with_no_retry_left_is_fail_with_its_hold` |
| (a), Binance's policy | `polyoxide-binance/src/shared/policy.rs:47`, `shared::policy::decide`'s arm for a 429 with no retry left (which `UsdmRetryPolicy` calls) holds the weight budget for its `Retry-After`, or to the next UTC minute | `hold: None` in that arm, so a 429 holds only when a retry is left | `polyoxide-binance/tests/mock_usdm.rs:616` `a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request`; `polyoxide-binance/tests/mock_usdm.rs:652` `a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute` |
| (b) `Retry-After` only extends a wait, never shortens it | `polyoxide-venue/src/retry_after.rs:45`, `retry_delay`'s `requested.map_or(computed, \|requested\| requested.max(computed))`, which `RetryConfig::retry_delay` calls | `requested.unwrap_or(computed)` | `polyoxide-core/src/client.rs:353` `retry_after_below_our_own_backoff_does_not_shorten_the_wait`; `polyoxide-venue/src/retry_after.rs:103` `a_requested_wait_only_ever_lengthens_the_backoff` |
| (b), a zero `Retry-After` | `polyoxide-venue/src/retry_after.rs:29` (`secs <= 0.0`), `polyoxide-venue/src/retry_after.rs:35` (the zero filter) and `polyoxide-venue/src/retry_after.rs:45`: the one parser, since DRIFT R4, refuses a zero twice | `secs < 0.0` at the first, `Some(wait)` unfiltered at the second and `requested.unwrap_or(computed)` at the third, which together obey Cloudflare's zero verbatim; the first two alone leave the tests passing, and the third alone is row (b)'s mutant | `polyoxide-core/src/client.rs:353`; `polyoxide-core/src/client.rs:372` `retry_after_zero_still_backs_off_exponentially_across_attempts`; `polyoxide-polymarket/tests/mock_data.rs:1515` `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop`; `polyoxide-polymarket/tests/mock_data.rs:1538`; `polyoxide-polymarket/tests/mock_data.rs:1562`; `polyoxide-venue/src/retry_after.rs:55` `every_parser_row` |
| (b), a socket reconnect (DRIFT R11) | `polyoxide-ws/src/backoff.rs:84`, `Backoff::take_at_least`'s `self.take().max(floor.min(self.max))`: a refusal's `Retry-After` only extends the reconnect delay, up to the ceiling. The kit's `Supervisor` calls it with `Protocol::retry_after`, and rtds and sports at their own reconnect waits | `self.take()`, the floor ignored | `polyoxide-ws/src/backoff.rs:207` `a_floor_extends_the_delay_and_never_shortens_it_drift_r11`; `polyoxide-ws/src/backoff.rs:223` `a_floor_past_the_ceiling_waits_the_ceiling_drift_r11`; `polyoxide-ws/src/supervisor/tests.rs:753` `a_reconnect_waits_out_a_refusals_retry_after_drift_r11`; `polyoxide-ws/src/supervisor/tests.rs:775` `a_retry_after_past_the_ceiling_waits_the_ceiling_drift_r11`; `polyoxide-ws/src/supervisor/tests.rs:795` `a_start_in_an_outage_waits_out_the_first_refusals_retry_after_drift_r11`; `polyoxide-polymarket/src/perps/ws/builder.rs:730` `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`; `polyoxide-binance/tests/supervision_usdm_kit.rs:437` `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`; `polyoxide-polymarket/tests/supervision_rtds.rs:327` `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`; `polyoxide-polymarket/src/sports/supervised.rs:484` `a_reconnect_waits_out_a_refusals_retry_after_drift_r11` |
| (c) Cooldowns only extend | `polyoxide-core/src/hold.rs:99`, `Hold::extend` keeps the later deadline | Assign `*slot = Some(until)` unconditionally | `polyoxide-polymarket/tests/polymarket_limits.rs:1036` `a_shorter_cooldown_never_cuts_a_longer_one_short`; `polyoxide-core/src/hold.rs:165` `a_hold_never_shortens` |
| (c), a hold extended mid-wait | `polyoxide-core/src/hold.rs:105-120`, `Hold::wait` re-reads the deadline after each sleep | `return` after the `sleep_until` at `polyoxide-core/src/hold.rs:118`, so it sleeps once | `polyoxide-core/src/hold.rs:180` `a_hold_extended_mid_wait_is_honoured_in_full` |
| (c), the re-check after a bucket wait (AD-23) | `polyoxide-core/src/rate_limit.rs:657-659`, `RateLimiter::acquire` waits out the hold again after its buckets | Delete that second `wait` | `polyoxide-core/src/rate_limit.rs:910` `a_hold_set_during_a_bucket_wait_is_honoured` |
| (d) `quota()` leaves depth at one token, with no `allow_burst` | `polyoxide-core/src/rate_limit.rs:236-239` | Append `.allow_burst(NonZeroU32::new(count).unwrap())` | `polyoxide-core/src/rate_limit.rs:316` `no_quota_admits_more_than_its_published_count_in_one_window`; `polyoxide-core/src/rate_limit.rs:333` `every_quota_reserves_headroom_below_the_published_count`; `polyoxide-polymarket/tests/polymarket_limits.rs:21` `every_configured_bucket_satisfies_the_quota_it_publishes` |
| (e) `classify_order_kill` needs both the order kind and the kill token | `polyoxide-polymarket/src/clob/error.rs:116` | `&&` → `\|\|` after `m.contains("fak order")` | `polyoxide-polymarket/src/clob/error.rs:459` `test_classify_requires_both_tokens` |
| (e), case | `polyoxide-polymarket/src/clob/error.rs:113`, the message is lowercased before matching | `let m = message.to_string();` | `polyoxide-polymarket/src/clob/error.rs:384` `test_classify_recognizes_verbatim_venue_messages`; `polyoxide-polymarket/src/clob/error.rs:396` `test_classify_preserves_message_verbatim`; `polyoxide-polymarket/src/clob/error.rs:404` `test_classify_is_case_insensitive`; `polyoxide-polymarket/src/clob/error.rs:417` `test_classify_tolerates_curly_apostrophe_in_fok_message`; `polyoxide-polymarket/tests/mock_clob.rs:3530` `fak_unmatched_maps_to_typed_error_not_generic_validation`; `polyoxide-polymarket/tests/mock_clob.rs:3564` `fok_unfilled_maps_to_typed_error_not_generic_validation` |
| (e), only a 400 is classified | `polyoxide-polymarket/src/clob/error.rs:156`, clob's decode, `From<ApiError>`, classifies an `ApiError::Response` only when its status is 400 | Drop the arm's `if response.status.as_u16() == 400` guard, so every response's message is classified | `polyoxide-polymarket/tests/mock_clob.rs:3628` `fak_prose_on_non_400_status_is_not_reclassified`; `polyoxide-polymarket/src/clob/error.rs:498` `the_decode_splits_out_a_kill_only_on_a_400`; `polyoxide-polymarket/src/clob/error.rs:678` `the_decode_splits_out_a_heartbeat_mismatch_only_on_a_400` |
| (f) `observe` runs on every response, the last attempt included | `polyoxide-core/src/send.rs:84`, before the policy decides | Call `observe` only when `retries_left > 0` | `polyoxide-polymarket/tests/send_loop.rs:252` `observe_sees_the_last_attempt` |
| (g) A retry sleeps at least the loop's floor, whatever wait the policy returns | `polyoxide-core/src/send.rs:97`, `floor.max(wait)` | Sleep `wait` | `polyoxide-polymarket/tests/send_loop.rs:278` `a_zero_wait_still_sleeps_the_floor`; `polyoxide-polymarket/tests/send_loop.rs:322` `the_429_hold_is_retry_delay_zero_not_the_attempts_wait` |
| (i) The per-signer layer's buckets hold their published burst, as governor's `allow_burst` did before Story 3.3 | `polyoxide-polymarket/src/shared/signer_limit.rs:278`, where each signer `CapacityBucket` takes the tier's burst as its capacity | `let capacity = 1;`, a bucket of one token | `polyoxide-polymarket/src/shared/signer_limit.rs:531` `a_batch_within_capacity_is_admitted`; `polyoxide-polymarket/src/shared/signer_limit.rs:540` `adopting_a_higher_tier_admits_a_batch_that_was_impossible`; `polyoxide-polymarket/src/shared/signer_limit.rs:556` `the_order_and_cancel_buckets_are_independent`; `polyoxide-polymarket/src/shared/signer_limit.rs:578` `batch_cost_is_charged_in_full_not_as_one_request` |
| (j) A window quota aims at 90% of its published count, the reserve measured on `/closed-positions` | `polyoxide-core/src/rate_limit.rs:269`, `RESERVED_FRACTION`, and `polyoxide-core/src/rate_limit.rs:261`, where `sustained_slots` takes the reserve off | `RESERVED_FRACTION = 20` at the first, which fails only the first test; `let target = count;` at the second | `polyoxide-core/src/rate_limit.rs:356` `a_published_150_per_10s_admits_135_per_window`; `polyoxide-core/src/rate_limit.rs:333` `every_quota_reserves_headroom_below_the_published_count` (the second mutant only) |
| (k) A `418` holds every request on the weight budget, for its `Retry-After` or two minutes, so no request is sent into a ban | `polyoxide-binance/src/shared/policy.rs:51`, `shared::policy::decide`'s 418 arm | `hold: None`, which fails the first two tests; `asked.unwrap_or(Duration::ZERO)`, which fails the third | `polyoxide-binance/tests/mock_usdm.rs:409` `a_418_is_not_retried_and_holds_the_next_request`; `polyoxide-binance/tests/mock_usdm.rs:707` `every_usdm_on_one_budget_is_held_by_one_ban`; `polyoxide-binance/src/usdm/policy.rs:160` `a_418_without_retry_after_holds_two_minutes` |
| (l) `GetDepth::limit` re-prices the route by its band, so a deeper spot book is charged its weight | `polyoxide-binance/src/spot/api/market.rs:167`, `GetDepth::limit`'s `.route(Route::Depth { limit: Some(limit) })` | Delete that call, leaving the route at `Depth { limit: None }` and the `limit` query still sent | `polyoxide-binance/tests/mock_spot.rs:923` `every_market_builder_charges_its_route`; `polyoxide-binance/tests/mock_spot.rs:977` `a_deep_depth_is_charged_its_band` |
| (m) Spot's socket requests go no faster than one per 500 ms, the pace its probe held for 45 s where one per 250 ms was closed with 1008 "Too many requests" | `polyoxide-binance/src/spot/ws/mod.rs:85`, `MIN_REQUEST_INTERVAL`, which `SpotStreams` hands to the shared bare connection and to the kit's `configure` | `Duration::from_millis(250)` | `polyoxide-binance/src/spot/ws/mod.rs:146` `the_limits_are_the_measured_ones`; `polyoxide-binance/src/spot/ws/protocol.rs:131` `the_stream_set_is_one_path_with_the_measured_limits`; `polyoxide-binance/src/spot/ws/client.rs:185` `the_documented_limits_are_the_measured_ones`; `polyoxide-binance/src/spot/ws/client.rs:232` `requests_are_paced_at_500_ms`; `polyoxide-binance/tests/supervision_spot.rs:187` `a_membership_change_waits_500_ms_for_the_one_before` |
| (n) A Protocol V2 order is hashed under domain version `"3"` | `polyoxide-polymarket/src/clob/core/eip712.rs:225`, `order_exchange`'s V2 arm returns `"3"` | `"2"` in place of `"3"` | `polyoxide-polymarket/src/clob/core/eip712.rs:1701` `each_case_signs_at_the_version_and_exchange_py_sdk_chose`; `polyoxide-polymarket/src/clob/core/eip712.rs:1719` `the_domain_separator_and_struct_hash_match_py_sdk`; `polyoxide-polymarket/src/clob/core/eip712.rs:1799` `a_v2_deposit_wallet_session_key_reproduces_py_sdks_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1784` `a_v2_deposit_wallet_owner_reproduces_py_sdks_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1738` `the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id`; `polyoxide-polymarket/src/clob/core/eip712.rs:1770` `plain_cases_reproduce_py_sdks_signatures`; `polyoxide-polymarket/tests/mock_clob.rs:5211` `create_order_for_a_v2_id_reads_the_tick_size_never_neg_risk_and_signs_for_exchange_v3`; `polyoxide-polymarket/tests/trading_clob.rs:1936` `place_on_a_protocol_v2_key_names_the_order_by_its_exchange_v3_hash` |
| (o) A Protocol V2 order is signed for ExchangeV3, never the CTF or neg-risk exchange, whatever `neg_risk` says | `polyoxide-polymarket/src/clob/core/eip712.rs:225`, `order_exchange`'s V2 arm takes `Chain::exchange_v3`'s address | the address the V1 path would pick, `version` left `"3"` | `polyoxide-polymarket/src/clob/core/eip712.rs:1701` `each_case_signs_at_the_version_and_exchange_py_sdk_chose`; `polyoxide-polymarket/src/clob/core/eip712.rs:1719` `the_domain_separator_and_struct_hash_match_py_sdk`; `polyoxide-polymarket/src/clob/core/eip712.rs:1784` `a_v2_deposit_wallet_owner_reproduces_py_sdks_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1799` `a_v2_deposit_wallet_session_key_reproduces_py_sdks_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1770` `plain_cases_reproduce_py_sdks_signatures`; `polyoxide-polymarket/src/clob/core/eip712.rs:1738` `the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id`; `polyoxide-polymarket/tests/mock_clob.rs:5211` `create_order_for_a_v2_id_reads_the_tick_size_never_neg_risk_and_signs_for_exchange_v3`; `polyoxide-polymarket/tests/trading_clob.rs:1936` `place_on_a_protocol_v2_key_names_the_order_by_its_exchange_v3_hash` |
| (p) A Deposit Wallet order's envelope is hashed under the order's own protocol: a V2 order's app domain is ExchangeV3 at version `"3"` | `polyoxide-polymarket/src/clob/core/eip712.rs:361`, `deposit_wallet::envelope_digest` takes the version from the order's token id | the version always `"2"` | `polyoxide-polymarket/src/clob/core/eip712.rs:1738` `the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id` |
| (q) The id rule's reserved-bit mask is bits 40 to 103, tested for zero | `polyoxide-polymarket/src/shared/protocol.rs:52` (the shift) and `polyoxide-polymarket/src/shared/protocol.rs:53` (the zero test), `is_v2_position_id`'s mask | `<< 41` then `<< 39` at the first line, `!= U256::ZERO` for `== U256::ZERO` at the second | `polyoxide-polymarket/src/shared/protocol.rs:129` `the_rule_is_the_conjunction_of_py_sdks_two_answers` (all three mutants); and, under `<< 39` and `!=` alone: `polyoxide-polymarket/src/clob/core/eip712.rs:1701` `each_case_signs_at_the_version_and_exchange_py_sdk_chose`; `polyoxide-polymarket/src/clob/core/eip712.rs:1823` `a_v2_id_is_refused_where_exchange_v3_is_not_deployed`; `polyoxide-polymarket/src/clob/core/eip712.rs:1719` `the_domain_separator_and_struct_hash_match_py_sdk`; `polyoxide-polymarket/src/clob/core/eip712.rs:1784` `a_v2_deposit_wallet_owner_reproduces_py_sdks_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1738` `the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id`; `polyoxide-polymarket/src/clob/core/eip712.rs:1770` `plain_cases_reproduce_py_sdks_signatures`; `polyoxide-polymarket/src/clob/core/eip712.rs:1842` `signing_a_v2_id_on_amoy_is_refused_before_any_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1799` `a_v2_deposit_wallet_session_key_reproduces_py_sdks_signature`; `polyoxide-polymarket/src/shared/protocol.rs:159` `the_four_fixture_ids_classify_as_the_venue_issues_them`; `polyoxide-polymarket/tests/mock_clob.rs:5282` `a_v2_id_on_an_amoy_client_is_refused_at_signing_and_nothing_is_posted`; `polyoxide-polymarket/tests/mock_clob.rs:5211` `create_order_for_a_v2_id_reads_the_tick_size_never_neg_risk_and_signs_for_exchange_v3`; `polyoxide-polymarket/tests/mock_clob.rs:5152` `balance_asset_types_follow_py_sdks_table`; `polyoxide-polymarket/tests/trading_clob.rs:1936` `place_on_a_protocol_v2_key_names_the_order_by_its_exchange_v3_hash` |
| (r) The id rule needs the decode as well as the mask, so toy ids such as `1`, `100` and `123456` stay V1 | `polyoxide-polymarket/src/shared/protocol.rs:53`, `is_v2_position_id`'s conjunction | the decode condition dropped, so `1` is V2 | `polyoxide-polymarket/src/clob/account/mod.rs:725` `sign_order_uses_the_deposit_wallet_target`; `polyoxide-polymarket/src/clob/core/eip712.rs:1216` `envelope_digest_matches_py_sdk_for_both_exchanges`; `polyoxide-polymarket/src/clob/core/eip712.rs:1537` `a_deposit_wallet_orders_hash_composes_py_sdks_domain_and_contents_hashes`; `polyoxide-polymarket/src/clob/core/eip712.rs:1573` `the_hash_follows_the_orders_exchange_and_refuses_an_unsupported_chain`; `polyoxide-polymarket/src/clob/core/eip712.rs:1557` `a_plain_orders_hash_is_the_digest_a_plain_signature_covers`; `polyoxide-polymarket/src/clob/core/eip712.rs:1290` `sign_order_as_owner_reproduces_py_sdk_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1305` `sign_order_as_session_key_reproduces_py_sdk_signature`; `polyoxide-polymarket/src/clob/core/eip712.rs:1369` `sign_order_as_uses_the_neg_risk_exchange_in_digest_and_trailer`; `polyoxide-polymarket/src/clob/core/eip712.rs:820` `order_digest_differs_by_chain`; `polyoxide-polymarket/src/clob/core/eip712.rs:806` `order_digest_uses_correct_exchange_for_neg_risk`; `polyoxide-polymarket/src/clob/core/eip712.rs:561` `v2_order_signature_is_deterministic`; `polyoxide-polymarket/src/shared/protocol.rs:129` `the_rule_is_the_conjunction_of_py_sdks_two_answers`; `polyoxide-polymarket/src/shared/protocol.rs:170` `toy_ids_stay_v1_though_py_sdks_mask_alone_accepts_them`; `polyoxide-polymarket/tests/mock_clob.rs:1873` `create_market_order_for_a_deposit_wallet_sets_maker_signer_and_type3`; `polyoxide-polymarket/tests/mock_clob.rs:1735` `create_order_for_a_deposit_wallet_sets_maker_signer_and_type3`; `polyoxide-polymarket/tests/mock_clob.rs:1530` `place_order_signs_and_submits_v2_body_with_builder_code`; `polyoxide-polymarket/tests/trading_clob.rs:1231` `a_resting_post_only_order_is_open_and_carries_the_request`; `polyoxide-polymarket/tests/trading_clob.rs:1451` `the_tick_size_and_neg_risk_are_looked_up_only_when_the_options_lack_them` |
| (s) A V2 id's balance is read as `CONDITIONAL-V2` | `polyoxide-polymarket/src/clob/api/account.rs:38`, `balance_allowance`'s asset type | `CONDITIONAL` for `CONDITIONAL-V2` | `polyoxide-polymarket/tests/mock_clob.rs:5152` `balance_asset_types_follow_py_sdks_table` |
| (t) `create_order` never asks `/neg-risk` about a V2 id | `polyoxide-polymarket/src/clob/client.rs:489`, `get_market_metadata`'s skip | the V2 test removed | `polyoxide-polymarket/tests/mock_clob.rs:5282` `a_v2_id_on_an_amoy_client_is_refused_at_signing_and_nothing_is_posted`; `polyoxide-polymarket/tests/mock_clob.rs:5211` `create_order_for_a_v2_id_reads_the_tick_size_never_neg_risk_and_signs_for_exchange_v3`; `polyoxide-polymarket/tests/trading_clob.rs:1936` `place_on_a_protocol_v2_key_names_the_order_by_its_exchange_v3_hash` |
| (u) A market's trading ids are chosen by `version`, never by which field is present | `polyoxide-polymarket/src/gamma/types.rs:357-364`, `Market::outcome_ids`'s choice of the field in the `V1` arm (357) and the `V2` arm (364) | the field chosen by presence: `position_ids` when `Some`, else `clob_token_ids`, in one arm for both versions | `polyoxide-polymarket/tests/outcome_ids_gamma.rs:333` `the_field_the_version_names_must_be_present`; `polyoxide-polymarket/tests/outcome_ids_gamma.rs:299` `clob_token_ids_that_are_not_a_json_array_of_strings_are_malformed`; `polyoxide-polymarket/tests/outcome_ids_gamma.rs:317` `the_field_the_version_does_not_name_is_never_read`; `polyoxide-polymarket/tests/outcome_ids_gamma.rs:168` `the_version_picks_the_field_when_both_are_present`; `polyoxide-polymarket/tests/outcome_ids_gamma.rs:92` `the_selector_agrees_with_py_sdk_on_every_vector`; `polyoxide-polymarket/tests/outcome_ids_gamma.rs:138` `each_refused_vector_is_refused_for_its_reason` |
| (v) A Router condition id is 62 hex digits, or 64 ending `00` or `01` | `polyoxide-polymarket/src/relay/types.rs:622`, `parse_v2_condition_id`'s last-byte check | any last byte accepted | `polyoxide-polymarket/src/relay/deposit_wallet.rs:622` `v2_condition_ids_narrow_as_py_sdk_does` |
| (w) A redeem sends the outcome index (0 or 1), never CTF's index set (1 or 2) | `polyoxide-polymarket/src/relay/deposit_wallet.rs:270`, `router_redeem_calldata`'s outcome-index argument | `1 << outcome_index` | `polyoxide-polymarket/src/relay/deposit_wallet.rs:696` `router_redeem_calldata_matches_py_sdk_for_both_outcomes`; `polyoxide-polymarket/src/relay/deposit_wallet.rs:724` `the_router_redeem_batch_digest_matches_py_sdk`; `polyoxide-polymarket/tests/mock_relay.rs:2516` `v2_calls_target_the_router_with_py_sdks_calldata`; `polyoxide-polymarket/tests/mock_relay.rs:2630` `submit_v2_redeem_posts_the_router_call_for_a_proxy`; `polyoxide-polymarket/tests/mock_relay.rs:2669` `submit_v2_redeem_posts_the_router_call_for_a_deposit_wallet`; `polyoxide-polymarket/tests/mock_relay.rs:2583` `submit_v2_redeem_posts_the_router_call_for_a_safe` |
| (x) Control: the order of the id rule's two conditions changes no answer | `polyoxide-polymarket/src/shared/protocol.rs:53`, `is_v2_position_id`'s conjunction | the operands of `&&` swapped | none: the mutant SURVIVES, every test passes |

`polyoxide-polymarket/tests/polymarket_limits.rs:1054` `a_cooldown_extended_mid_wait_is_honoured_in_full`
held row (c), a hold extended mid-wait, until Story 3.3. It drives `RateLimiter::acquire`, whose
re-check after its buckets now also waits out an extended hold, so it passes under that row's
mutant; `Hold`'s own test holds the row, and the re-check has a row of its own.

`polyoxide-polymarket/src/clob/error.rs:428` `test_classify_does_not_capture_neighbouring_400s` fails
under none of the mutants above. It pins the 400s that neighbour the kill outcomes, so keep
it, but do not count it as holding any of these rules.

Row (g) is held by the two loop tests alone. Under its mutant
`polyoxide-polymarket/tests/mock_data.rs:1515` fails only when both of its holds jitter short (once
in three runs), and `polyoxide-polymarket/tests/mock_perps.rs:544`
`a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff` never does: a 429's
hold, `retry_delay(0)`, already keeps the next attempt back for as long as the floor would,
and on perps the route's own bucket does too. Neither counts as holding (g).
