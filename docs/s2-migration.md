# Migrating to polyoxide 0.40 (S2)

polyoxide 0.40.0 is stage S2 of the multi-venue restructure, the renames stage. Every crate
merge, module move and rename the restructure needs ships in this one release, so a consumer
rewrites its imports once:

- `polyoxide-clob`, `-gamma`, `-data`, `-relay`, `-perps`, `-rtds` and `-sports` are the
  modules of one new crate, `polyoxide-polymarket`, each behind the feature of its name.
- `polyoxide-core` names no venue: Polymarket's tables, retry policy, throttle and signers
  moved to `polyoxide_polymarket::shared`.
- `polyoxide-binance` keeps its name and puts everything under its one module, `usdm`.
- The `polyoxide` umbrella is multi-venue: `polyoxide::polymarket` and `polyoxide::binance`,
  behind features named for each venue and module.
- The CLI's commands are `polyoxide <venue> <module> <verb…>`.

A move breaks the old path, with no shim, alias or re-export (AD-16), so the compiler finds
every call site. Behaviour does not change: test bodies moved unchanged (AD-12), and the
behaviour notes below are the exceptions, mostly in the CLI. This guide is for code on 0.39.x.

**Where each path went.** [`rename-manifest.md`](rename-manifest.md) gives every public path,
feature and CLI command that 0.40 moves or removes a row: the old path, then the new one or
the reason it went. A row covers its path and every path below it, so
`polyoxide_gamma` → `polyoxide_polymarket::gamma` covers all of gamma. The expanded map,
[`rename-map.tsv`](rename-map.tsv), lists every one of the 10,693 entries those rows resolve,
the 23 features among them, one per line and tab-separated: the old path, its new path or
the reason it went, its kind (`struct`, `function`, `feature` and so on), and `resolved`, or
`resolved, doc-hidden` for the 62 doc-hidden ones. It was generated from the manifest by
`api_removals.py manifest --expand` and is frozen with this release. The CLI's commands are
in the manifest only.

`polyoxide-polymarket` is on crates.io for the first time. The seven retired crates are
published once more at 0.40.0 as tombstones: each has no dependencies, and its build stops
with a `compile_error!` naming `polyoxide-polymarket`, the feature and the manifest. A
requirement of `"0.39"` never resolves to a tombstone, but a looser one (`"0"`, `">=0.39"`)
does. No old version is yanked, so a pin to 0.39 keeps building.

## Upgrade checklist

1. Replace each retired crate with `polyoxide-polymarket`, with `default-features = false`
   and the features of the modules you used ([Crates and features](#crates-and-features)).
   Keep every other `polyoxide*` dependency on `0.40` together.
2. Rewrite the imports: `polyoxide_<crate>::…` is `polyoxide_polymarket::<crate>::…`, and
   `polyoxide::polyoxide_<crate>::…` is `polyoxide::polymarket::<crate>::…`.
3. Rename the errors that take their module's name: `DataApiError` → `DataError`, clob's
   `ws::WebSocketError` → `ws::ClobWsError`, Binance's `BinanceError` → `usdm::UsdmError`.
   The unified client's `PolymarketError` is gone; `Polymarket::new` and
   `PolymarketBuilder::build` return `BuildError`.
4. Binance: import from `polyoxide_binance::usdm`, and name `features = ["usdm"]` or
   `["usdm-ws"]` if you turned the defaults off.
5. The umbrella: turn on `polymarket-<module>` and `binance-<module>` features where you
   turned on `clob`, `gamma`, `ws` and the rest ([The umbrella](#the-umbrella)).
6. Code that built its own client on core's Polymarket items: import them from
   `polyoxide_polymarket::shared`, with the feature of a module that uses them.
7. Move `RUST_LOG` filters that name an old crate ([Logging](#logging)).
8. CLI scripts: rewrite each command ([The CLI](#the-cli)), and pass `--format json` where
   you piped market or user events into `jq`.
9. Python: nothing changes.

## Crates and features

| 0.39 | 0.40 | Module |
|---|---|---|
| `polyoxide-clob = "0.39"` | `polyoxide-polymarket = { version = "0.40", default-features = false, features = ["clob", "gamma"] }` | `polyoxide_polymarket::clob` |
| `polyoxide-clob`'s `ws` | `polyoxide-polymarket`'s `clob-ws`, which turns `clob` on | `polyoxide_polymarket::clob::ws` |
| `polyoxide-gamma = "0.39"` | `… features = ["gamma"]` | `polyoxide_polymarket::gamma` |
| `polyoxide-data = "0.39"` | `… features = ["data"]` | `polyoxide_polymarket::data` |
| `polyoxide-relay = "0.39"` | `… features = ["relay"]` | `polyoxide_polymarket::relay` |
| `polyoxide-perps = "0.39"` | `… features = ["perps"]` | `polyoxide_polymarket::perps` |
| `polyoxide-perps`'s `ws` | `polyoxide-polymarket`'s `perps-ws`, which turns `perps` on | `polyoxide_polymarket::perps::ws` |
| `polyoxide-rtds = "0.39"` | `… features = ["rtds"]` | `polyoxide_polymarket::rtds` |
| `polyoxide-sports = "0.39"` | `… features = ["sports"]` | `polyoxide_polymarket::sports` |
| `polyoxide-rtds`'s `test-fixtures`, `polyoxide-sports`'s `test-server` | `polyoxide-polymarket`'s `test-server`, with `rtds` or `sports` | each module's doc-hidden `fixtures` |
| `polyoxide-gamma`'s and `polyoxide-data`'s `specta` | `polyoxide-polymarket`'s `specta`, with `gamma` or `data` | |
| `polyoxide-clob`'s and `polyoxide-relay`'s `keychain` | `polyoxide-polymarket`'s `keychain`, with `clob` or `relay` | |
| `polyoxide-binance`'s `ws` | `usdm-ws`, which turns `usdm` on | `polyoxide_binance::usdm::ws` |

Several modules in one build share one dependency line:
`features = ["clob", "gamma", "data", "clob-ws"]`.

**`default-features = false` matters.** `polyoxide-polymarket`'s defaults are `clob`, `gamma`
and `data`. Copying `features = ["gamma"]` without `default-features = false` also builds
clob, with its signing stack (`alloy`, `hmac`, `sha2`), and data. For the socket feeds that
undoes what the old crates gave you: `rtds` or `sports` alone, with the defaults off, builds
no `polyoxide-core`, reqwest or alloy, as `polyoxide-rtds` and `polyoxide-sports` did. Every
module README's install line carries it.

**`clob` no longer implies `gamma`.** `polyoxide-clob`'s defaults turned on its proxy-wallet
lookup through Gamma; in `polyoxide-polymarket` the lookup is built only with both `clob`
and `gamma`, which is why the table names both. With `clob` alone, `ClobBuilder::gamma` and
`ClobError::Gamma` do not exist, and a proxy-signature order with no `funder` is refused with
a validation error, whose message names `polyoxide-polymarket/gamma` where it named
`polyoxide-clob/gamma`.

**`keychain` builds `keyring` with any HTTP module.** It is `polyoxide-core?/keychain`, which
cargo cannot limit to `clob` and `relay`, so `gamma` or `data` with `keychain` builds
`keyring` too. `rtds` or `sports` with it builds none.

**Binance with its defaults off builds nothing.** In 0.39 `polyoxide-binance`'s defaults were
empty and the REST client always built; the client is the `usdm` feature now, on by default.
`test-server` no longer turns the streams on, so a build that wants `usdm::ws::fixtures`
names `usdm-ws` and `test-server`. The crate root re-exports nothing:
`polyoxide_binance::Usdm` is `polyoxide_binance::usdm::Usdm`, and so for `UsdmBuilder`,
`WeightBudget`, `error` and `weight`.

**Core's Polymarket items.** `polyoxide_core::polymarket` (the five limit tables,
`ClobThrottle`, `clob_throttle`, `PolymarketRetryPolicy`, `signer_cost` and the layer ids) is
`polyoxide_polymarket::shared`, and core's `signer_limit`, `session_signer` and `auth`
modules, `Signer`, `Base64Format`, `current_timestamp`, `SignerLimiter`, `Tier`,
`TradingBucket`, `TradingRequest`, `RateLimitStatus`, `BurstCapacityExceeded`,
`DepositWalletRole` and `SessionSignerScope` are there under their own names. In 0.39 any
build of core had them. Now each is built by the features of the modules that use it: the
retry policy by any HTTP module (`clob`, `gamma`, `data`, `relay`, `perps`), each table by its
own module's, the throttle and the signer layer by `clob`, and the HMAC signer and the
session-signer types by `clob` or `relay`. Core keeps the venue-neutral parts: the send loop
and its hooks, `WindowQuotaTable`, `CapacityBucket`, `Hold` and the keychain.

## The umbrella

The crate root holds `venue` (`polyoxide-venue`, always on, so
`use polyoxide::venue::Classify;` needs no direct dependency), `polymarket` (the whole of
`polyoxide-polymarket`), `binance` (the whole of `polyoxide-binance`) and `prelude`, and
nothing else.

| 0.39 feature | 0.40 feature |
|---|---|
| `clob` | `polymarket-clob` |
| `gamma` | `polymarket-gamma` |
| `data` | `polymarket-data` |
| `ws` | `polymarket-clob-ws`, which turns `polymarket-clob` on |
| `rtds` | `polymarket-rtds` |
| `perps` | `polymarket-perps` |
| `perps-ws` | `polymarket-perps-ws`, which turns `polymarket-perps` on |
| `sports` | `polymarket-sports` |
| | `polymarket-relay`, new: the relay was not in the umbrella |
| | `binance`, `binance-usdm`, `binance-usdm-ws`, new |
| `default` (`clob`, `gamma`, `data`) | `default` is `polymarket`, the same three modules |
| `full` | `full`: every module of both venues, and now `keychain` too |
| `keychain` | `keychain` |

`full` grew: it builds the relay with alloy's HTTP provider, the Binance crate and
`keyring` (with its Secret Service client on Linux). The umbrella's `polymarket-clob` without
`polymarket-gamma` builds no proxy-wallet lookup, where 0.39's `clob` alone did; the default
has both.

The unified client moved into the venue crate: `polyoxide::Polymarket` and
`PolymarketBuilder` are `polyoxide::polymarket::Polymarket` and `PolymarketBuilder`, built
with `clob`, `gamma` and `data`. Their setters are unchanged.

The prelude keeps its 0.39 names, but for the three renamed errors: `DataError` for
`DataApiError`, `ws::ClobWsError` for `ws::WebSocketError`, and `BuildError` for
`PolymarketError`. It adds Binance's `Usdm`, `UsdmBuilder`, `UsdmError` and `WeightBudget`
under `binance-usdm`, and `UsdmWs`, `UsdmWsBuilder`, `UsdmWsError`, `StreamName` and
`Event as UsdmEvent` under `binance-usdm-ws`. `BuildError` and `StreamName` name no venue, so
a second glob import exporting either is ambiguous where both are used.

## Consumer impact

What 0.40 changes beyond the paths and features above. cargo-semver-checks lists the changes
it can see on paths that stayed in the manifest's `changed-in-place` section; the rest come
from the stories that made them.

### Errors

- **`KeychainError` is `#[non_exhaustive]`** and has a new variant,
  `Invalid { service, field, reason }`, classed `InvalidRequest`, which only
  `keychain::store` returns; its `reason` never quotes the value. A `match` on it needs a
  wildcard arm. This is the one lint in the `changed-in-place` section.
- **`BuildError` is narrower than `PolymarketError`, and `#[non_exhaustive]`.** Its variants
  are `Clob`, `Gamma` and `Data`, each wrapping the error of the client that could not be
  built and classed as that error, with the same `Display` text as before
  (`CLOB error: …`, `Gamma error: …`, `Data error: …`). `PolymarketError`'s `Perps` variant
  was never returned and its `Config` never built; code that used it as a general error type,
  for `?` over perps calls, needs an error type of its own.
- **The renamed errors keep their text and classes.** `DataError`, `ClobWsError` and
  `UsdmError` display and classify as `DataApiError`, `WebSocketError` and `BinanceError` did,
  so alerting on messages or classes is unaffected.
- **`ClobError::Gamma`** exists only with `gamma` ([Crates and features](#crates-and-features)).

### Logging

Each module's own log lines take its new module path as their target, so a `RUST_LOG` or
`EnvFilter` directive naming an old crate matches nothing. `polyoxide_polymarket` matches
every line of Polymarket's modules. The send loop's retry, hold, decode and `Request failed`
lines stay under `polyoxide_core`, and the supervised sockets' reconnect lines under
`polyoxide_ws`, so the soak harnesses' `Retriable status <code> on <path>, retry <n> after
<ms>ms` is unchanged. Gamma and data log nothing of their own.

| 0.39 target | 0.40 target | Lines |
|---|---|---|
| `polyoxide_core::signer_limit` | `polyoxide_polymarket::shared::signer_limit` | DEBUG `adopting rate limit tier …` |
| `polyoxide_clob::ws::client` | `polyoxide_polymarket::clob::ws::client` | TRACE `Skipping non-event message` |
| `polyoxide_relay::client` | `polyoxide_polymarket::relay::client` | ERROR `Request to <endpoint> failed with status …` |
| `polyoxide_perps::ws::client`, `polyoxide_perps::ws::protocol` | `polyoxide_polymarket::perps::ws::client`, `polyoxide_polymarket::perps::ws::protocol` | WARN `subscribe rate limited, retrying`, and the bare tier's lines |
| `polyoxide_rtds::event`, `polyoxide_rtds::supervisor` | `polyoxide_polymarket::rtds::event`, `polyoxide_polymarket::rtds::supervisor` | WARN `RTDS connection lost, reconnecting` and its kin, DEBUG skipped frames |
| `polyoxide_sports::client`, `polyoxide_sports::supervised` | `polyoxide_polymarket::sports::client`, `polyoxide_polymarket::sports::supervised` | WARN `lost the sports feed; reconnecting` and its kin, INFO `reconnected to the sports feed` |
| `polyoxide_binance::error` | `polyoxide_binance::usdm::error` | WARN `418: IP banned: <body>`; a filter on `polyoxide_binance` still matches it |

Binance's stream lines were already under `polyoxide_binance::usdm::ws` and did not move.

### The CLI

The command moves are the manifest's `## Commands` rows: `polyoxide gamma …` is
`polyoxide polymarket gamma …`, and each socket is its module's `stream` verb
(`polyoxide ws market` is `polyoxide polymarket clob stream market`, `ws prices` is
`polymarket rtds stream`, `ws sports` is `polymarket sports stream`, `ws binance` is
`binance usdm stream`). No old command is kept as an alias. Beyond the moves:

- **`--format pretty` means one line per event on market and user.** In 0.39 `pretty`, the
  default, printed indented JSON, and `summary` printed one line per event. Now `pretty`,
  still the default, prints what `summary` did, `summary` is refused, and `json` prints one
  compact object per line as before. A script that piped the default into `jq` needs
  `--format json`. Every stream takes the same two formats.
- **A stream error that cannot be skipped exits non-zero** on market, user and prices, which
  printed `Error: …` and exited 0. A frame the client cannot decode is skipped with a marker
  on stderr, where 0.39 ended the run on it.
- **Ctrl+C is no longer caught.** Market, user and prices installed a handler that closed the
  socket and printed a count; the process now ends with the signal.
- **A closed stdout ends a stream quietly**, where market, user and prices panicked on the
  broken pipe (`… | head -1`).
- **`-t` fires under a backlog** on every stream, where sports' and Binance's never fired
  while events kept arriving.
- **The stderr markers changed text** on market, user and prices: `Reached N update(s)`,
  `Timeout reached` and `The feed ended` (it said `Connection closed`); events still go to
  stdout.
- **Every list argument splits on commas.** Market's asset ids, user's market ids, the
  `--filter`s, RTDS's `--symbol` and `prices download --token-id` took one value per
  occurrence; they also take `a,b` now, trimmed, with an empty entry dropped.
- **`polyoxide credentials …` is `polyoxide polymarket credentials …`**, and the `relay` kind
  is `builder`, with the same service and flags. `store` checks every value before it writes
  any (an empty or repeated field, a private key that does not parse, a relayer API key
  without its address or the reverse) and writes nothing on a refusal, where 0.39 wrote what
  it was given. `show` prints the relayer API key's address, the one field that is not a
  secret, and `error: <message>` for a backend failure; it never prints a secret. Each flag's
  help is now generated from its field.

### Python

Nothing a caller sees changes: the module, class and exception names, and the mapping from
error class to exception, are 0.39's. The wheel builds the CLOB with its Gamma lookup, which
its read-only `Clob` never reaches.

### Keychain

Stored credentials load as before: the services are still `polyoxide-clob` and
`polyoxide-relay`, with the same field names, so `Account::from_keychain()` and
`BuilderAccount::from_keychain()` read what 0.39 stored. `polyoxide_core::keychain` adds the
`StoredCredential` trait (`SERVICE`, `FIELDS`, `validate`), `Field`, and the generic `store`,
`show` and `delete_all` over it; `clob::Account` and `relay::BuilderAccount` implement it.
The library's `save_*_to_keychain` functions still write without checks; only `store`
validates. `Credentials`, `BuilderConfig` and `ApiCredentials` keep their `String` fields.

## What did not change

- Request and socket behaviour: every moved test kept its body, and every request sends the
  same headers (`accept: */*`, `accept-encoding: gzip`, `host`).
- Error classes, `is_fault()`, `retry_after()` and every error's `Display` text.
- The keychain services and field names, and the Python package's names.
- The MSRV, 1.91.

## Known gaps in 0.40

- The unified client builds two Gamma clients: `PolymarketBuilder::build` does not hand its
  Gamma to the CLOB, so the proxy-wallet lookup ignores `gamma_base_url`. 0.39 did the same.
- Gamma's and sports' `game_id` still use three types (`i64` on `ListEvents::game_id`,
  `Option<u64>` on `Event` and `MatchUpdate`, `Option<String>` on `Market`), so a caller
  casts.
- `ClobWsError::{ConnectionClosed, Authentication}` are never constructed; S3 removes them.
- `polyoxide_polymarket::shared::current_timestamp`, a venue-neutral clock, stays in `shared`;
  S3 may add one to `polyoxide-venue` beside `UnixMillis`.
