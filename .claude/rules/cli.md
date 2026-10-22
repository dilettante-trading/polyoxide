---
paths:
  - "polyoxide-cli/**"
---

# The CLI (`polyoxide-cli`)

It queries Polymarket's APIs and Binance USDⓈ-M market data, and depends on the component crates
directly, never on the umbrella (CLAUDE.md's crate list names the features).

## The command tree

`polyoxide <venue> <module> <verb…>`: `polyoxide polymarket` holds `gamma`, `data`, `clob`,
`rtds`, `sports` and, with `keychain`, `credentials`; `polyoxide binance` holds `usdm`;
`completions` stays at the top. The library follows the tree,
`polyoxide_cli::commands::{polymarket, binance, completions}`, beside the shared `stream` and
`credentials` modules, and the root of `commands` re-exports nothing.

v0.39.0's transport-first groups (`ws`, and `gamma`, `data`, `clob` and `credentials` at the top)
are gone with no aliases. `docs/rename-manifest.md`'s hand-written `## Commands` maps each old
command to its new one, and `src/main.rs`'s `every_new_command_in_the_manifest_parses` fails
unless each new command there parses as a subcommand path and each old one is gone.

## Every list argument splits on commas

A clap `Vec` field needs `value_delimiter`, not a value parser that returns a `Vec`: the latter
compiles and parses, then panics when the field is read. Every v1 `data` list flag shipped that
way, and no test caught it because the parse tests never passed those flags.
`src/main.rs`'s `every_list_argument_splits_on_commas` walks the clap tree and fails on any `Vec`
argument without `value_delimiter = ','`, positionals and `--filter`s included; its one exception
is the hidden `_ignored` of `data positions closed`, which swallows what follows the removed
command. Each list argument also has a parse test that passes a comma list.

## Every socket is its module's `stream` verb, on one runner

`polymarket clob stream market` and `… user`, `polymarket rtds stream`, `polymarket sports stream`
and `binance usdm stream` each describe their socket as a `commands::stream::Feed` (what each
event and each error means), and `stream::run` owns the rest:

- `-n`, counted in printed events and checked before each wait, so a feed that goes quiet after
  the last wanted event still ends; `-t`, where a timeout too large to add means none;
  "The feed ended"; a `BrokenPipe` on stdout ending the run quietly; and `excerpt()`, a frame cut
  to one escaped stderr line.
- Every stream's `--format` is the one `stream::OutputFormat`: `pretty` (one human-readable line
  per event) or `json` (one compact object per line). Events go to stdout; markers, filters and
  skipped frames to stderr.
- A frame a feed cannot read is skipped with a marker; any other stream error ends the run with
  that error.
- No stream installs a Ctrl+C handler, and the CLI does not depend on `ctrlc`.
- Each command's `run_with(args, events, out, err)` takes any event stream, so
  `tests/ws_sports.rs`, `tests/ws_binance.rs` and `tests/stream_polymarket.rs` (market, user and
  prices) drive every flag with scripted events, and `stream`'s own tests pin the runner's four
  behaviours.
- `polymarket sports stream` takes comma-separated `--league` and `--game` filters and
  `--changes-only`, and is driven with the sports module's captured frames.
- `binance usdm stream` streams through the supervised tier of `polyoxide-binance`'s `usdm`
  module. `--symbol` and `--kind` are comma-separated, and each kind is streamed for each symbol;
  `--all-tickers` and `--all-mark-prices` add the two array streams. Outage markers go to stderr,
  and `--format json` prints each frame's envelope. `tests/ws_binance.rs` drives it with the
  crate's captured frames (`polyoxide_binance::usdm::ws::fixtures`, features `usdm-ws` and
  `test-server`).

## Logging

The CLI installs a `tracing` subscriber that writes to stderr at `warn`, and `RUST_LOG` overrides
the level. That is how the libraries' recoveries become visible: a reconnect being retried, or a
429 being waited out. Without it, a feed retrying a dead host looks like a quiet one.

## `polymarket clob prices download`

A bulk, resumable, rate-limited downloader for CLOB historical price data
(`GET /prices-history`) that writes per-market CSV/JSONL/Parquet dataset files plus a
`manifest.jsonl`. Parquet output requires building the CLI with the `parquet` feature. Its
`--format csv|jsonl|parquet` is a different `OutputFormat`, in `polymarket::clob::prices::types`.

## `polymarket data`

The group reads Data API v2, except `data health`, which stays on v1's `/` because `/v2/status`
reports data freshness, not liveness.

- Listing commands print the v2 `{data, pagination}` envelope and page with `--cursor`, `--all`
  (JSONL, flushed per page) and `--max-pages`; the cursor to resume from goes to stderr when a walk
  stops early. `--offset` is refused with a pointer to `--cursor`.
- `data traded` prints the `/v2/user-stats` object, or `null` for an unknown wallet.
- `DataCommand::run_with` takes the client and the output writers, and `tests/data_v2.rs` uses it
  to run real arguments against mock servers serving the `data` module's v2 fixtures
  (`polyoxide-polymarket/tests/fixtures/data/v2/`).

## `polymarket credentials <store|show|delete> <kind>` (feature `keychain`)

The command's kinds, checks and output rules are in `keychain.md`.

The CLI's live tests are `tests/live_api.rs`.
