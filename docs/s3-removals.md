---
baseline: v0.40.0
---

# S3 removals

Every public item that stage S3 removes, against the S3 start tag above, the S2 release.
S3 is for additions (the venue traits, a supervised clob socket, the Kalshi skeleton), and
its removals are few, such as clob's never-constructed `ClobWsError::{ConnectionClosed,
Authentication}`. The list holds nothing yet: the story that removes an item adds its entry
here when it lands. Each removal goes with no shim or re-export (AD-16), so prader-rs and
every other consumer migrates from this list. [`s1-removals.md`](s1-removals.md) is S1's, and
S2's moves are in [`rename-manifest.md`](rename-manifest.md).

The commit after the S2 release moves the workspace to stage S3, and from it CI's removals
job runs `python3 scripts/api_removals.py check --baseline v0.40.0`, which reads this list
because the stage is S3. It fails on any removal that cargo-semver-checks reports and this
list does not hold, and on any path under "Doc-hidden paths consumers import" that no
longer compiles; the release's semver job imports the same paths. The list is cumulative
until the S3 release: an entry stays after its PR merges.

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

Nothing yet.

## Doc-hidden paths consumers import

cargo-semver-checks treats `#[doc(hidden)]` items as private, and 0.51.0 does not check type
aliases, so it never reports their removal. The removals job imports each item below, and
the release's semver job does too. Each entry names the features it needs after its path,
and the entries of one crate that name the same features are built together, with those
features and no others, so an entry cannot compile through another's. `a::b::{C, D}` lists
`a::b::C` and `a::b::D`.

The items are the ones the CLI's tests and each crate's own tests import, at their S2 paths:
the live entries of [`rename-manifest.md`](rename-manifest.md)'s list at the S2 release.

- `polyoxide_binance::usdm::ws::fixtures::{AGG_TRADE, ALL, ALL_MARK_PRICES, ALL_TICKERS, BOOK_TICKER, KLINE, MARK_PRICE, PARTIAL_DEPTH, TICKER}` `usdm-ws` `test-server`
- `polyoxide_polymarket::perps::ws::{frame_from_text_for_tests, incoming_from_text_for_tests, IncomingForTests}` `perps-ws`
- `polyoxide_polymarket::rtds::fixtures::{REJECTED_SUBSCRIPTION, TWAP_THIRTY_UPDATE}` `rtds` `test-server`
- `polyoxide_polymarket::sports::fixtures::{CRICKET, CRICKET_FINISHED, ESPORTS, LEAGUE_WITH_SPACE, SOCCER, TENNIS_EVENT_STATE}` `sports` `test-server`

The public type aliases, imported the same way:

- `polyoxide_polymarket::clob::DynSigner` `clob`
- `polyoxide_polymarket::clob::account::DynSigner` `clob`
- `polyoxide_polymarket::data::v2::PageStream` `data`
- `polyoxide_polymarket::perps::ws::SupervisedPerpsWs` `perps-ws`
- `polyoxide_polymarket::relay::DynSigner` `relay`
