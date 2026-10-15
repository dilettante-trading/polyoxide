---
baseline: v0.39.0
---

# S2 rename manifest

Every public path that stage S2 moves or removes, against the S2 start tag above. A move
breaks the old path, with no shim, alias or re-export (AD-16), so prader-rs and every other
consumer migrates from this file: each old path has a row saying where it went, or why it
went. The S2 release does not merge until every entry has one.
[`s2-migration.md`](s2-migration.md) explains the moves and what changed beyond them, and
[`rename-map.tsv`](rename-map.tsv) expands the rows into every entry they resolve. Both, and
this file, are final as of the S2 release, 0.40.0.

CI's removals job runs
`python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict`. It builds rustdoc
JSON for every publishable crate with a library, at the baseline and at the commit, with all
features and with `#[doc(hidden)]` items, and lists every public path of each: modules,
types, traits, functions, constants, statics, type aliases and exported macros, and their
variants, fields, inherent methods and associated constants, the members of a type reached
through a type alias among them, along every public re-export, a sibling crate's included. A
path the baseline has and the commit does not is an **entry**, and so is a baseline feature,
`<crate>/<feature>`, that the commit does not have. The job resolves each entry through the
rows below and prints each finding as a warning, and `--strict`, which the job passes since
Story 5.12, fails it on any unresolved entry, stale row or stale region.

## How to read a row

A row is one line: an old path in backticks, then `→`, then either the new path in
backticks or a reason. Text after a new path is commentary, such as the story that made the
move. A row with neither is refused.

- `polyoxide_clob` → `polyoxide_polymarket::clob` covers every path under the old crate: a
  row applies to its own path and to every path below it, split at `::`.
- `polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` overrides the
  crate's row for that type and its members, such as `DataApiError::V2`: the row with the
  longest old path wins.
- `polyoxide_x::Gone` → removed: never constructed says why a path went, and what a
  consumer uses instead if anything does.

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
paths. Data's error enum takes its module's name, `DataError` (AD-15's `<Module>Error`).
The umbrella re-exports `polyoxide_polymarket` in their place, as `polyoxide::polymarket`
since Story 5.7.

- `polyoxide_gamma` → `polyoxide_polymarket::gamma` Story 5.3, feature `gamma`.
- `polyoxide_data` → `polyoxide_polymarket::data` Story 5.3, feature `data`.
- `polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` Story 5.3, renamed for its module.
- `polyoxide_data::error::DataApiError` → `polyoxide_polymarket::data::error::DataError` Story 5.3, renamed for its module.
- `polyoxide::polyoxide_gamma` → `polyoxide::polymarket::gamma` Story 5.3 moved the module; the `polyoxide::polymarket` path and its `polymarket-gamma` feature date from Story 5.7.
- `polyoxide::polyoxide_data` → `polyoxide::polymarket::data` Story 5.3 moved the module; the `polyoxide::polymarket` path and its `polymarket-data` feature date from Story 5.7.
- `polyoxide::polyoxide_data::DataApiError` → `polyoxide::polymarket::data::DataError` Story 5.3 renamed it for its module; the `polyoxide::polymarket` path dates from Story 5.7.
- `polyoxide::polyoxide_data::error::DataApiError` → `polyoxide::polymarket::data::error::DataError` Story 5.3 renamed it for its module; the `polyoxide::polymarket` path dates from Story 5.7.
- `polyoxide::prelude::DataApiError` → `polyoxide::prelude::DataError` Story 5.3, renamed for its module.

Story 5.4 folded `polyoxide-clob` and `polyoxide-relay` into `polyoxide-polymarket` as the
modules `clob` and `relay`, each under the feature of its name, keeping their own module
paths. The CLOB's sockets, `clob::ws`, are under `clob-ws`, and their error enum takes its
module's name, `ClobWsError` (AD-15's `<Module>WsError`). `clob` no longer implies `gamma`:
the proxy-wallet lookup through Gamma is built only with both. The umbrella re-exports
`polyoxide_polymarket` in place of `polyoxide_clob`, as `polyoxide::polymarket` since Story 5.7.

- `polyoxide_clob` → `polyoxide_polymarket::clob` Story 5.4, feature `clob`; `ws` under `clob-ws`.
- `polyoxide_relay` → `polyoxide_polymarket::relay` Story 5.4, feature `relay`.
- `polyoxide_clob::ws::WebSocketError` → `polyoxide_polymarket::clob::ws::ClobWsError` Story 5.4, renamed for its module.
- `polyoxide::polyoxide_clob` → `polyoxide::polymarket::clob` Story 5.4 moved the module; the `polyoxide::polymarket` path, its `polymarket-clob` feature and `ws` under `polymarket-clob-ws` date from Story 5.7.
- `polyoxide::polyoxide_clob::ws::WebSocketError` → `polyoxide::polymarket::clob::ws::ClobWsError` Story 5.4 renamed it for its module; the `polyoxide::polymarket` path dates from Story 5.7.
- `polyoxide::prelude::ws::WebSocketError` → `polyoxide::prelude::ws::ClobWsError` Story 5.4, renamed for its module.

Story 5.5 folded `polyoxide-perps`, `polyoxide-rtds` and `polyoxide-sports` into
`polyoxide-polymarket` as the modules `perps`, `rtds` and `sports`, each under the feature of
its name, keeping their own module paths and error names. Perps' sockets, `perps::ws`, are
under `perps-ws`. `rtds` and `sports` are sockets alone and build no `polyoxide-core`. The
umbrella re-exports `polyoxide_polymarket` in their place, as `polyoxide::polymarket` since
Story 5.7, under its `polymarket-perps`, `polymarket-rtds` and `polymarket-sports` features. Their `tracing` targets moved with them: what logged under
`polyoxide_perps::…`, `polyoxide_rtds::…` and `polyoxide_sports::…` now logs under
`polyoxide_polymarket::perps::…`, `polyoxide_polymarket::rtds::…` and
`polyoxide_polymarket::sports::…`, so a `RUST_LOG` filter naming an old crate matches nothing.

- `polyoxide_perps` → `polyoxide_polymarket::perps` Story 5.5, feature `perps`; `ws` under `perps-ws`.
- `polyoxide_rtds` → `polyoxide_polymarket::rtds` Story 5.5, feature `rtds`.
- `polyoxide_sports` → `polyoxide_polymarket::sports` Story 5.5, feature `sports`.
- `polyoxide::polyoxide_perps` → `polyoxide::polymarket::perps` Story 5.5 moved the module; the `polyoxide::polymarket` path, its `polymarket-perps` feature and `ws` under `polymarket-perps-ws` date from Story 5.7.
- `polyoxide::polyoxide_rtds` → `polyoxide::polymarket::rtds` Story 5.5 moved the module; the `polyoxide::polymarket` path and its `polymarket-rtds` feature date from Story 5.7.
- `polyoxide::polyoxide_sports` → `polyoxide::polymarket::sports` Story 5.5 moved the module; the `polyoxide::polymarket` path and its `polymarket-sports` feature date from Story 5.7.

Story 5.6 laid `polyoxide-binance` out as `polyoxide-polymarket` is: its error and weight
budget moved under its one module, `usdm`, which is behind the feature of its name, and the
crate root re-exports nothing. `usdm` is on by default, so a consumer with
`default-features = false`, which built the REST client before, now builds nothing and names
`features = ["usdm"]`. The error enum takes its module's name, `UsdmError` (AD-15's
`<Module>Error`). The streams, `usdm::ws`, are under `usdm-ws`, and `test-server` no longer
turns them on: a build that wants the captured frames, `usdm::ws::fixtures`, names
`usdm-ws` and `test-server` both. The decode's `418` warning moved with its file: what logged
under `polyoxide_binance::error` now logs under `polyoxide_binance::usdm::error`, which a
`RUST_LOG` filter on `polyoxide_binance` still matches.

- `polyoxide_binance::error` → `polyoxide_binance::usdm::error` Story 5.6, feature `usdm`.
- `polyoxide_binance::error::BinanceError` → `polyoxide_binance::usdm::error::UsdmError` Story 5.6, renamed for its module.
- `polyoxide_binance::weight` → `polyoxide_binance::usdm::weight` Story 5.6, feature `usdm`.
- `polyoxide_binance::BinanceError` → `polyoxide_binance::usdm::UsdmError` Story 5.6, renamed for its module.
- `polyoxide_binance::Usdm` → `polyoxide_binance::usdm::Usdm` Story 5.6: the crate root re-exports nothing.
- `polyoxide_binance::UsdmBuilder` → `polyoxide_binance::usdm::UsdmBuilder` Story 5.6: the crate root re-exports nothing.
- `polyoxide_binance::WeightBudget` → `polyoxide_binance::usdm::WeightBudget` Story 5.6: the crate root re-exports nothing.

Story 5.7 made the `polyoxide` umbrella multi-venue. It holds `polyoxide::venue` (always on),
`polyoxide::polymarket` and `polyoxide::binance`, behind features named for each venue and
module, and a `prelude`; its crate root holds nothing else. Polymarket's features are
`polymarket`, the default, which turns on `polymarket-clob`, `polymarket-gamma` and
`polymarket-data`, and one `polymarket-<module>` per module, relay's included; Binance's are
`binance`, `binance-usdm` and `binance-usdm-ws`. `full` now covers every module of both
venues and `keychain`, which keeps its name. The unified client moved into
`polyoxide-polymarket`, built with `clob`, `gamma` and `data`, and `Polymarket::new` and
`PolymarketBuilder::build` now return its `BuildError`, whose `Clob`, `Gamma` and `Data`
variants wrap the error of the client that could not be built and classify as it does. The
builder's setters are unchanged. `PolymarketError` is removed: its `Perps` variant was never
returned by anything, and its `Config` variant was never built. The prelude keeps its names,
re-exporting `Polymarket`, `PolymarketBuilder` and `BuildError` from their new home, and adds
Binance's clients, errors and streams.

- `polyoxide::Polymarket` → `polyoxide::polymarket::Polymarket` Story 5.7, features `polymarket-clob`, `polymarket-gamma` and `polymarket-data`; `new` returns `BuildError`.
- `polyoxide::PolymarketBuilder` → `polyoxide::polymarket::PolymarketBuilder` Story 5.7, features `polymarket-clob`, `polymarket-gamma` and `polymarket-data`; `build` returns `BuildError`.
- `polyoxide::PolymarketError` → removed in Story 5.7: the unified client's builder returns `polyoxide::polymarket::BuildError`, and nothing else returned it.
- `polyoxide::prelude::PolymarketError` → removed in Story 5.7: the prelude re-exports `BuildError`, which the unified client's builder returns.

Stories 5.8 and 5.9 put the CLI on the venue axis: its commands are
`polyoxide <venue> <module> <verb…>`, listed under `## Commands` below, and its library
follows them. `polyoxide_cli::commands` holds `polymarket`, `binance`, `completions`, the
shared `stream` runner and, with `keychain`, the shared `credentials` command, and its root
re-exports nothing. Gamma, Data and the CLOB's commands moved under `polymarket` unchanged,
and the CLOB's gained `Stream`. Each socket is its module's `stream` verb: sports' and
Binance's modules keep their `SportsArgs`, `BinanceArgs`, `run` and `run_with`, and every
stream prints in the one `stream::OutputFormat`, `pretty` or `json`. The credentials command
is each venue's, generic over the kinds its library describes with core's
`keychain::StoredCredential`.

- `polyoxide_cli::commands::gamma` → `polyoxide_cli::commands::polymarket::gamma` Story 5.8: `polyoxide polymarket gamma`.
- `polyoxide_cli::commands::data` → `polyoxide_cli::commands::polymarket::data` Story 5.8: `polyoxide polymarket data`.
- `polyoxide_cli::commands::clob` → `polyoxide_cli::commands::polymarket::clob` Story 5.8: `polyoxide polymarket clob`, whose `ClobCommand` gains `Stream`.
- `polyoxide_cli::commands::ws::sports` → `polyoxide_cli::commands::polymarket::sports` Story 5.8: `polyoxide polymarket sports stream`.
- `polyoxide_cli::commands::ws::sports::OutputFormat` → `polyoxide_cli::commands::stream::OutputFormat` Story 5.8: one format for every stream.
- `polyoxide_cli::commands::ws::binance` → `polyoxide_cli::commands::binance::usdm` Story 5.8: `polyoxide binance usdm stream`.
- `polyoxide_cli::commands::ws::binance::OutputFormat` → `polyoxide_cli::commands::stream::OutputFormat` Story 5.8: one format for every stream.
- `polyoxide_cli::commands::ws` → removed in Story 5.8: each socket is its module's `stream` command, `polymarket::clob::stream` for the market and user channels, `polymarket::rtds` for prices, `polymarket::sports` and `binance::usdm`; market's and user's `--format summary` is the default `pretty`.
- `polyoxide_cli::commands::WsCommand` → removed in Story 5.8 with the `ws` group: each socket is its module's `stream` command.
- `polyoxide_cli::commands::ClobCommand` → removed in Story 5.8: the root of `commands` re-exports nothing; the command is `polyoxide_cli::commands::polymarket::clob::ClobCommand`.
- `polyoxide_cli::commands::DataCommand` → removed in Story 5.8: the root of `commands` re-exports nothing; the command is `polyoxide_cli::commands::polymarket::data::DataCommand`.
- `polyoxide_cli::commands::GammaCommand` → removed in Story 5.8: the root of `commands` re-exports nothing; the command is `polyoxide_cli::commands::polymarket::gamma::GammaCommand`.
- `polyoxide_cli::commands::CompletionsCommand` → removed in Story 5.8: the root of `commands` re-exports nothing; the command is `polyoxide_cli::commands::completions::CompletionsCommand`.
- `polyoxide_cli::commands::CredentialsCommand` → removed in Story 5.9: the root of `commands` re-exports nothing, and the credentials command is each venue's.
- `polyoxide_cli::commands::credentials::CredentialsCommand` → removed in Story 5.9: each venue's `credentials` command is `polyoxide_cli::commands::credentials::Credentials` over that venue's kinds, Polymarket's `clob` and `builder` (v0.39.0's `relay`).

## Features

- `polyoxide-gamma/default` → `polyoxide-polymarket/gamma` Story 5.3: the module's own feature builds what the crate's defaults did.
- `polyoxide-gamma/specta` → `polyoxide-polymarket/specta` Story 5.3, with `gamma`.
- `polyoxide-data/default` → `polyoxide-polymarket/data` Story 5.3: the module's own feature builds what the crate's defaults did.
- `polyoxide-data/specta` → `polyoxide-polymarket/specta` Story 5.3, with `data`.
- `polyoxide-clob/default` → `polyoxide-polymarket/clob` Story 5.4, with `gamma`: the crate's defaults turned on its proxy-wallet lookup through Gamma, which `clob` no longer implies.
- `polyoxide-clob/gamma` → `polyoxide-polymarket/gamma` Story 5.4, with `clob`: both build the proxy-wallet lookup.
- `polyoxide-clob/ws` → `polyoxide-polymarket/clob-ws` Story 5.4, which turns on `clob`.
- `polyoxide-clob/keychain` → `polyoxide-polymarket/keychain` Story 5.4, with `clob`.
- `polyoxide-relay/default` → `polyoxide-polymarket/relay` Story 5.4: the module's own feature builds what the crate's defaults did.
- `polyoxide-relay/keychain` → `polyoxide-polymarket/keychain` Story 5.4, with `relay`.
- `polyoxide-perps/default` → `polyoxide-polymarket/perps` Story 5.5: the module's own feature builds what the crate's defaults did.
- `polyoxide-perps/ws` → `polyoxide-polymarket/perps-ws` Story 5.5, which turns on `perps`.
- `polyoxide-rtds/test-fixtures` → `polyoxide-polymarket/test-server` Story 5.5, with `rtds`; DRIFT R6 renamed it to the name every other crate's had.
- `polyoxide-sports/test-server` → `polyoxide-polymarket/test-server` Story 5.5, with `sports`.
- `polyoxide-binance/ws` → `polyoxide-binance/usdm-ws` Story 5.6, which turns on `usdm`.
- `polyoxide/clob` → `polyoxide/polymarket-clob` Story 5.7.
- `polyoxide/gamma` → `polyoxide/polymarket-gamma` Story 5.7.
- `polyoxide/data` → `polyoxide/polymarket-data` Story 5.7.
- `polyoxide/ws` → `polyoxide/polymarket-clob-ws` Story 5.7, which turns on `polymarket-clob`.
- `polyoxide/rtds` → `polyoxide/polymarket-rtds` Story 5.7.
- `polyoxide/perps` → `polyoxide/polymarket-perps` Story 5.7.
- `polyoxide/perps-ws` → `polyoxide/polymarket-perps-ws` Story 5.7, which turns on `polymarket-perps`.
- `polyoxide/sports` → `polyoxide/polymarket-sports` Story 5.7.

## Commands

The CLI's command tree, which no public-API diff can see, in the same form as the rows
above. Stories 5.8 and 5.9 put it on the venue axis, `polyoxide <venue> <module> <verb…>`,
with `stream` as the verb of every socket; `completions` stays at the top. A command keeps
its flags unless its row says otherwise, and every list flag also takes a comma-separated
list. No old command is kept as an alias. `polyoxide-cli`'s tests read this section and fail
unless each new command below parses, and each old one is gone.

- `polyoxide gamma` → `polyoxide polymarket gamma` Story 5.8.
- `polyoxide data` → `polyoxide polymarket data` Story 5.8.
- `polyoxide clob` → `polyoxide polymarket clob` Story 5.8.
- `polyoxide clob prices download` → `polyoxide polymarket clob prices download` Story 5.8.
- `polyoxide ws` → removed in Story 5.8: each socket streams under its venue and module, with the verb `stream`, as the rows below say.
- `polyoxide ws market` → `polyoxide polymarket clob stream market` Story 5.8: the default `--format pretty` prints one summary line per event, where it printed indented JSON; a stream error it cannot skip exits non-zero, where it printed `Error:` and exited 0; and Ctrl+C is no longer caught, so the process ends with the signal.
- `polyoxide ws user` → `polyoxide polymarket clob stream user` Story 5.8: the default `--format pretty` prints one summary line per event, where it printed indented JSON; a stream error it cannot skip exits non-zero, where it printed `Error:` and exited 0; and Ctrl+C is no longer caught, so the process ends with the signal.
- `polyoxide ws prices` → `polyoxide polymarket rtds stream` Story 5.8: a stream error it cannot skip exits non-zero, where it printed `Error:` and exited 0; and Ctrl+C is no longer caught, so the process ends with the signal.
- `polyoxide ws sports` → `polyoxide polymarket sports stream` Story 5.8.
- `polyoxide ws binance` → `polyoxide binance usdm stream` Story 5.8.
- `polyoxide ws market --format summary` → removed in Story 5.8: `pretty`, the default, prints what `summary` did, one line per event; `json` prints each event as one compact object per line, and indented JSON is gone.
- `polyoxide ws user --format summary` → removed in Story 5.8: `pretty`, the default, prints what `summary` did, one line per event; `json` prints each event as one compact object per line, and indented JSON is gone.
- `polyoxide credentials` → `polyoxide polymarket credentials` Story 5.9, with the `keychain` feature.
- `polyoxide credentials store clob` → `polyoxide polymarket credentials store clob` Story 5.9, with the `keychain` feature.
- `polyoxide credentials show clob` → `polyoxide polymarket credentials show clob` Story 5.9, with the `keychain` feature.
- `polyoxide credentials delete clob` → `polyoxide polymarket credentials delete clob` Story 5.9, with the `keychain` feature.
- `polyoxide credentials store relay` → `polyoxide polymarket credentials store builder` Story 5.9, with the `keychain` feature: the kind is named for what it holds; its service, `polyoxide-relay`, and its flags are unchanged.
- `polyoxide credentials show relay` → `polyoxide polymarket credentials show builder` Story 5.9, with the `keychain` feature.
- `polyoxide credentials delete relay` → `polyoxide polymarket credentials delete builder` Story 5.9, with the `keychain` feature.

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

- `polyoxide_binance::usdm::ws::fixtures::{AGG_TRADE, ALL, ALL_MARK_PRICES, ALL_TICKERS, BOOK_TICKER, KLINE, MARK_PRICE, PARTIAL_DEPTH, TICKER}` `usdm-ws` `test-server`
- `polyoxide_polymarket::perps::ws::{frame_from_text_for_tests, incoming_from_text_for_tests, IncomingForTests}` `perps-ws`
- `polyoxide_polymarket::rtds::fixtures::{REJECTED_SUBSCRIPTION, TWAP_THIRTY_UPDATE}` `rtds` `test-server`
- `polyoxide_polymarket::sports::fixtures::{CRICKET, CRICKET_FINISHED, ESPORTS, LEAGUE_WITH_SPACE, SOCCER, TENNIS_EVENT_STATE}` `sports` `test-server`
- `polyoxide_polymarket::clob::DynSigner` `clob`
- `polyoxide_polymarket::clob::account::DynSigner` `clob`
- `polyoxide_polymarket::data::v2::PageStream` `data`
- `polyoxide_polymarket::perps::ws::SupervisedPerpsWs` `perps-ws`
- `polyoxide_polymarket::relay::DynSigner` `relay`

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
- `polyoxide-core enum_marked_non_exhaustive: enum KeychainError (src/keychain.rs)`
<!-- manifest:end changed-in-place -->
