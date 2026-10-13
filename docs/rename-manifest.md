---
baseline: v0.39.0
---

# S2 rename manifest

Every public path that stage S2 moves or removes, against the S2 start tag above. A move
breaks the old path, with no shim, alias or re-export (AD-16), so prader-rs and every other
consumer migrates from this file: each old path has a row saying where it went, or why it
went. The S2 release does not merge until every entry has one.

CI's removals job runs `python3 scripts/api_removals.py manifest --baseline v0.39.0`. It
builds rustdoc JSON for every publishable crate with a library, at the baseline and at the
commit, with all features and with `#[doc(hidden)]` items, and lists every public path of
each: modules, types, traits, functions, constants, statics, type aliases and exported
macros, and their variants, fields, inherent methods and associated constants, the members
of a type reached through a type alias among them, along every public re-export, a sibling
crate's included. A path the baseline has and the commit does not is an **entry**, and so is
a baseline feature, `<crate>/<feature>`, that the commit does not have. The job resolves
each entry through the rows below and reports what is left. Through S2 it fails on none of
it: it prints each finding as a warning, and Story 5.12 runs it with `--strict`, which
fails on any unresolved entry, stale row or stale region.

## How to read a row

A row is one line: an old path in backticks, then `→`, then either the new path in
backticks or a reason. Text after a new path is commentary, such as the story that made the
move. A row with neither is refused.

- `polyoxide_clob` → `polyoxide_polymarket::clob` covers every path under the old crate: a
  row applies to its own path and to every path below it, split at `::`.
- `polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` overrides the
  crate's row for that type and its members, such as `DataApiError::V2`: the row with the
  longest old path wins.
- `polyoxide_clob::ws::WebSocketError::ConnectionClosed` → removed: never constructed
  (Story 5.4) says why a path went, and what a consumer uses instead if anything does.

A row that maps to a new path must map each entry it covers to a path the commit has, of
the same kind, or the entry stays unresolved: listed as pointing nowhere, or as existing at
the commit as another kind. A public path mapped onto a doc-hidden one stays unresolved too,
until a reason row of its own says why it is hidden now. A row that covers no entry is
**stale**: its old path is not at the baseline, or every path it covers is still at the
commit. Feature rows work the same way under `## Features`, keyed `<crate>/<feature>`, or
`<crate>` alone for all of a crate's features, split at `/`.

## How to add a row

1. Read the removals job's log, or its `rename-manifest` artifact, or run the command above
   locally. Each unresolved entry is printed with its kind.
2. Add a row under `## Paths` or `## Features` that covers it. Prefer one row for a whole
   module or crate, and a row of its own for each item that does not follow it.
3. Do not edit the generated regions at the end. Only the hub, the integration session that
   merges each story, regenerates them, with `--write`, so two stories never fight over
   them.

## Paths

Story 5.2 moved Polymarket's code out of `polyoxide-core` into `polyoxide-polymarket`'s
`shared` module, which keeps core's names. Each item there is built by the features of the
modules that use it: the retry policy by any of `clob`, `gamma`, `data`, `relay` and
`perps`, each table by its own module's, the throttle and the signer layer by `clob`, and
the HMAC signer and the session-signer types by `clob` or `relay`.

- `polyoxide_core::polymarket` → `polyoxide_polymarket::shared` Story 5.2: the retry policy, the five tables, the CLOB's throttle, `signer_cost` and the layer ids.
- `polyoxide_core::signer_limit` → `polyoxide_polymarket::shared::signer_limit` Story 5.2, feature `clob`.
- `polyoxide_core::session_signer` → `polyoxide_polymarket::shared::session_signer` Story 5.2, feature `clob` or `relay`.
- `polyoxide_core::auth` → `polyoxide_polymarket::shared::auth` Story 5.2, feature `clob` or `relay`.
- `polyoxide_core::BurstCapacityExceeded` → `polyoxide_polymarket::shared::BurstCapacityExceeded` Story 5.2, feature `clob`.
- `polyoxide_core::RateLimitStatus` → `polyoxide_polymarket::shared::RateLimitStatus` Story 5.2, feature `clob`.
- `polyoxide_core::SignerLimiter` → `polyoxide_polymarket::shared::SignerLimiter` Story 5.2, feature `clob`.
- `polyoxide_core::Tier` → `polyoxide_polymarket::shared::Tier` Story 5.2, feature `clob`.
- `polyoxide_core::TradingBucket` → `polyoxide_polymarket::shared::TradingBucket` Story 5.2, feature `clob`.
- `polyoxide_core::TradingRequest` → `polyoxide_polymarket::shared::TradingRequest` Story 5.2, feature `clob`.
- `polyoxide_core::DepositWalletRole` → `polyoxide_polymarket::shared::DepositWalletRole` Story 5.2, feature `clob` or `relay`.
- `polyoxide_core::SessionSignerScope` → `polyoxide_polymarket::shared::SessionSignerScope` Story 5.2, feature `clob` or `relay`.
- `polyoxide_core::Signer` → `polyoxide_polymarket::shared::Signer` Story 5.2, feature `clob` or `relay`.
- `polyoxide_core::Base64Format` → `polyoxide_polymarket::shared::Base64Format` Story 5.2, feature `clob` or `relay`.
- `polyoxide_core::current_timestamp` → `polyoxide_polymarket::shared::current_timestamp` Story 5.2, feature `clob` or `relay`.

Story 5.3 folded `polyoxide-gamma` and `polyoxide-data` into `polyoxide-polymarket` as the
modules `gamma` and `data`, each under the feature of its name, keeping their own module
paths. Data's error enum takes its module's name, `DataError` (AD-1's `<Module>Error`).
Until Story 5.7 reshapes the umbrella, `polyoxide` re-exports `polyoxide_polymarket` in their
place.

- `polyoxide_gamma` → `polyoxide_polymarket::gamma` Story 5.3, feature `gamma`.
- `polyoxide_data` → `polyoxide_polymarket::data` Story 5.3, feature `data`.
- `polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` Story 5.3, renamed for its module.
- `polyoxide_data::error::DataApiError` → `polyoxide_polymarket::data::error::DataError` Story 5.3, renamed for its module.
- `polyoxide::polyoxide_gamma` → `polyoxide::polyoxide_polymarket::gamma` Story 5.3, feature `gamma`.
- `polyoxide::polyoxide_data` → `polyoxide::polyoxide_polymarket::data` Story 5.3, feature `data`.
- `polyoxide::polyoxide_data::DataApiError` → `polyoxide::polyoxide_polymarket::data::DataError` Story 5.3, renamed for its module.
- `polyoxide::polyoxide_data::error::DataApiError` → `polyoxide::polyoxide_polymarket::data::error::DataError` Story 5.3, renamed for its module.
- `polyoxide::prelude::DataApiError` → `polyoxide::prelude::DataError` Story 5.3, renamed for its module.

## Features

- `polyoxide-gamma/specta` → `polyoxide-polymarket/specta` Story 5.3, with `gamma`.
- `polyoxide-data/specta` → `polyoxide-polymarket/specta` Story 5.3, with `data`.

## Doc-hidden paths consumers import

cargo-semver-checks treats `#[doc(hidden)]` items as private, and 0.51.0 does not check type
aliases, so it never reports their removal. The removals job imports each item below, and
the release's semver job does too while the workspace is in stage S2. Each entry names the
features it needs after its path, and the entries of one crate that name the same features
are built together, with those features and no others, so an entry cannot compile through
another's. `a::b::{C, D}` lists `a::b::C` and `a::b::D`. The list starts as
[`s1-removals.md`](s1-removals.md)'s live entries.

A story that moves one of these paths changes its entry to the new path in the same commit,
and gives the old path a row above. To remove one, append `**Removed**` to its entry, then
the story and what a consumer uses instead. To remove one item of a `{...}` group, give it a
line of its own first.

- `polyoxide_binance::usdm::ws::fixtures::{AGG_TRADE, ALL, ALL_MARK_PRICES, ALL_TICKERS, BOOK_TICKER, KLINE, MARK_PRICE, PARTIAL_DEPTH, TICKER}` `test-server`
- `polyoxide_perps::ws::{frame_from_text_for_tests, incoming_from_text_for_tests, IncomingForTests}` `ws`
- `polyoxide_rtds::fixtures::{REJECTED_SUBSCRIPTION, TWAP_THIRTY_UPDATE}` `test-fixtures`
- `polyoxide_sports::fixtures::{CRICKET, CRICKET_FINISHED, ESPORTS, LEAGUE_WITH_SPACE, SOCCER, TENNIS_EVENT_STATE}` `test-server`
- `polyoxide_clob::DynSigner`
- `polyoxide_clob::account::DynSigner`
- `polyoxide_polymarket::data::v2::PageStream` `data`
- `polyoxide_perps::ws::SupervisedPerpsWs` `ws`
- `polyoxide_relay::DynSigner`

## Unresolved entries

Each entry no row resolves, with its kind, why it is unresolved, and whether it is
doc-hidden. Generated; do not edit.

<!-- manifest:begin unresolved -->
None.
<!-- manifest:end unresolved -->

## Stale rows

Each row that covers no entry, and why. Generated; do not edit.

<!-- manifest:begin stale -->
None.
<!-- manifest:end stale -->

## Changed in place

The changes cargo-semver-checks reports to a path that stays, such as a new required field
or a changed signature, each keyed as `<crate> <lint id>: <item> (<file>)`. The path diff
cannot see them, and no row resolves them; they are listed for the S2 release notes'
consumer-impact section. Generated; do not edit.

<!-- manifest:begin changed-in-place -->
None.
<!-- manifest:end changed-in-place -->
