---
baseline: v0.38.1
---

# S1 removals

Every public item that stage S1 removes, against the S1 start tag above. Consolidated items
break at their old paths, with no shim or re-export (AD-16), so prader-rs and every other
consumer migrates from this list.

CI's removals job runs `python3 scripts/api_removals.py check --baseline v0.38.1`. It fails
on any removal that cargo-semver-checks reports and this list does not hold, and on any path
under "Doc-hidden paths consumers import" that no longer compiles. The list is cumulative
until the S1 release: an entry stays after its PR merges.

To remove a public item in a PR:

1. Run the command above, or read the removals job's log. It prints each unlisted removal as
   a key, `<crate> <lint id>: <item> (<file>)`, where the file is relative to the crate. A
   crate deleted from the workspace is keyed `<crate> crate_missing`.
2. Add each key under "Removed" as one list entry, in backticks and exactly as printed, then
   the story and what a consumer uses instead. An entry with nothing after its key is
   refused.
3. To remove a path listed under "Doc-hidden paths consumers import", append `**Removed**`
   to its entry, then the story and what a consumer uses instead. To remove one item of a
   `{...}` group, give it a line of its own first.

## Removed

- `polyoxide-core inherent_method_missing: RateLimiter::clob_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::clob_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::gamma_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::gamma_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::data_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::data_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::relay_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::relay_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::perps_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::perps_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-clob module_missing: mod polyoxide_clob::request (src/request.rs)` Story 3.4: clob's namespaces build core's one request builder, `polyoxide_core::Request<T, polyoxide_clob::ClobError>`, which every namespace method now returns.
- `polyoxide-clob struct_missing: struct polyoxide_clob::request::Request (src/request.rs)` Story 3.4: use `polyoxide_core::Request<T, polyoxide_clob::ClobError>`, which every namespace method now returns, with the same `query`, `send` and `send_raw`.
- `polyoxide-clob enum_missing: enum polyoxide_clob::request::AuthMode (src/request.rs)` Story 3.4: nothing to name; each namespace method signs its request itself (L1 for key creation, L2 for the rest), and a signature produced elsewhere goes through `Clob::create_api_key_with_signature` or `Clob::derive_api_key_with_signature`.

## Doc-hidden paths consumers import

cargo-semver-checks treats `#[doc(hidden)]` items as private, so it never reports their
removal. The removals job imports each item below, and the release's semver job does too.
Each entry names the features it needs after its path, and the entries of one crate that
name the same features are built together, with those features and no others, so an entry
cannot compile through another's. `a::b::{C, D}` lists `a::b::C` and `a::b::D`.

The items are the ones the CLI's tests and each crate's own tests import:

- `polyoxide_binance::usdm::ws::fixtures::{AGG_TRADE, ALL, ALL_MARK_PRICES, ALL_TICKERS, BOOK_TICKER, KLINE, MARK_PRICE, PARTIAL_DEPTH, TICKER}` `test-server`
- `polyoxide_binance::usdm::ws::test_server::{Script, ScriptedServer}` `test-server`
- `polyoxide_perps::ws::test_server` `test-server`
- `polyoxide_perps::ws::{frame_from_text_for_tests, incoming_from_text_for_tests, IncomingForTests}` `ws`
- `polyoxide_rtds::fixtures::{REJECTED_SUBSCRIPTION, TWAP_THIRTY_UPDATE}` `test-fixtures`
- `polyoxide_rtds::test_server::{Script, ScriptedServer}` `test-fixtures`
- `polyoxide_sports::fixtures::{CRICKET, CRICKET_FINISHED, ESPORTS, LEAGUE_WITH_SPACE, SOCCER, TENNIS_EVENT_STATE}` `test-server`
- `polyoxide_sports::test_server::{Script, ScriptedServer}` `test-server`

cargo-semver-checks 0.51.0 does not check type aliases either, so the public ones are imported
the same way:

- `polyoxide_clob::DynSigner`
- `polyoxide_clob::account::DynSigner`
- `polyoxide_data::v2::PageStream`
- `polyoxide_relay::DynSigner`
