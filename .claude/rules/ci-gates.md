---
paths:
  - ".github/workflows/ci.yml"
  - ".github/scripts/tests/**"
  - "scripts/{api_removals,test_body_diff,venue_gate,runtime_gate,gen_registry,public_api}.py"
  - "ci/**"
  - "**/Cargo.toml"
  - "docs/{s1-removals,s3-removals,s3-migration,rename-manifest,MUTANTS}.md"
  - "docs/rename-map.tsv"
  - "flake.nix"
  - "polyoxide-wasm-smoke/**"
  - "scripts/wasm_*.sh"
---

# CI gates in detail

CLAUDE.md lists the ten jobs and the command each runs; this file says what each checks.

## package

`python3 scripts/publish_order.py check-manifests` fails when a publishable crate lacks a
description or a licence (cargo only warns about those), or a tombstone breaks its rules
(`release.md`); then `cargo publish --workspace --dry-run --no-verify --locked`; then
`for m in tombstones/*/Cargo.toml; do [ -e "$m" ] || continue; cargo package --no-verify --manifest-path "$m" || exit 1; done`,
which packages each tombstone without building it (none since S3), since the workspace dry run
cannot see them. The job skips `publish = false` members on its own, and catches a manifest fault
on the PR that introduces it rather than halfway through a release.

## features

1. `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`: a feature that
   compiles only alongside another fails.
2. Each HTTP module's header pin in its minimal build, one `-p` per command so their features
   never unify: `cargo test -p polyoxide-polymarket --no-default-features --features <m> --test headers_<m>`
   for clob, gamma, data, relay and perps; `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm`;
   `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`.
   The pin test derives the HTTP modules from cargo metadata: a venue crate's products whose
   feature builds `polyoxide-core`, so `rtds` and `sports` pin none.
3. The same pins under the umbrella's `full`,
   `cargo test -p polyoxide -p <venue crate> --features polyoxide/full --test headers_<m>`, which
   unifies every module of every venue into the pinned build. The pin test derives this step
   too, and fails unless `full` turns on each module of a crate the umbrella depends on and of
   every published venue crate (only a `publish = false` crate the umbrella does not depend on
   would be exempt; none is).
4. `mock_clob`'s `a_proxy_order_with_no_funder_is_refused_without_gamma` with `--features clob`
   alone, the one build that compiles clob's refusal of a proxy order with no funder (`clob` does
   not imply `gamma`); then `trading_clob` with `--features clob-ws,data`, whose three proxy tests
   refuse without `gamma` and are compiled only there.
5. `cargo check -p polyoxide-polymarket --no-default-features --features <m>,specta` for each HTTP
   module of a crate with `specta` (clob, gamma, data, relay and perps; the test derives them from
   cargo metadata): a module's specta extras (`specta?/chrono`, `specta?/rust_decimal`) are weak
   features that neither `--each-feature` (which builds `specta` alone) nor `--all-features`
   (which turns every extra on at once) builds apart.

## CI scripts (`.github/scripts/tests/`)

- `test_dependency_fences.py` fails when a member that declares a `venue` depends directly on
  `governor` (a venue crate paces its requests with core's throttle primitives), and when a member
  depends directly, in any dependency kind, on a crate another member owns: `reqwest` is
  `polyoxide-core`'s; `tokio-tungstenite` and `rustls` are `polyoxide-ws`'s. Each owner must
  declare its crate, so the check cannot pass vacuously.
- `test_classify_coverage.py`: `errors.md`.
- `test_dep_allowlists.py` fails when
  `cargo tree -p <crate> -e normal --target x86_64-unknown-linux-gnu`, with the features the file
  names (and `--no-default-features` for `polyoxide-polymarket`, `polyoxide-binance`,
  `polyoxide-kalshi` and `polyoxide`, whose files name every feature they build; the socket kit's
  bare file is its default build), differs from `ci/dep-allowlists/<crate>[@<features>].txt`,
  naming each crate that joined or left the tree; or when an allowlist admits a crate its
  deny-list refuses, naming the rule. After a deliberate change, regenerate the files with
  `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` and say why in the
  commit. The deny-lists (NFR2), applied after a file's features turn on the ones they imply:
  - `polyoxide-ws`, the socket-only deny-list: `reqwest`, `alloy*`, `polyoxide-core`, `governor`,
    `hmac`, `sha2`, `keyring` and `polyoxide-venue`.
  - `polyoxide-polymarket`: `reqwest`, `polyoxide-core`, `governor` and `sha2` only with an HTTP
    module (`clob`, `gamma`, `data`, `relay` or `perps`), which holds the socket modules `rtds`
    and `sports` to the socket-only tree; `alloy*` and `hmac` only with `clob` or `relay`;
    `keyring` only with `keychain`. Its files are `@gamma`, `@data`, `@perps`, `@perps-ws`,
    `@clob`, `@clob-ws`, `@relay`, `@clob,keychain`, `@rtds`, `@rtds,test-server`, `@sports` and
    `@sports,test-server`; `@perps` must hold `reqwest`, `@clob` and `@relay` `alloy`, and
    `@clob,keychain` `keyring`, so no rule can pass vacuously.
  - `polyoxide-binance`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with
    `usdm-ws` (files `@usdm` and `@usdm-ws`; `@usdm-ws` must hold `polyoxide-ws`). `rustls` is not
    on the rule, since core's HTTP client builds it for TLS.
  - `polyoxide-kalshi`: `reqwest`, `polyoxide-core`, `governor` and `sha2` only with
    `predictions`, which every file names; `alloy*`, `hmac` and `keyring` never; `polyoxide-ws`,
    `tokio-tungstenite` and `tungstenite` only with `predictions-ws` (files `@predictions` and
    `@predictions-ws`; the latter must hold `polyoxide-ws`).
  - `polyoxide`, per feature on its own names: the HTTP stack only with an HTTP module of any
    venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps`, `binance-usdm` or
    `kalshi-predictions`); `alloy*` and `hmac` only with `polymarket-clob` or `polymarket-relay`;
    `keyring` only with `keychain`. `@polymarket-rtds` and `@polymarket-sports` are the
    socket-only tree, `@polymarket-perps` and `@kalshi-predictions` must hold `reqwest`, and
    `@kalshi-predictions-ws` holds the socket kit.
- `test_dep_allowlists.py` also pins the browser build (spec D10): the files in
  `ci/dep-allowlists/wasm32-unknown-unknown/`, named as the host's are, hold
  `cargo tree -p <crate> -e normal --target wasm32-unknown-unknown` for spec D1's crates and
  features, which the test's `WASM_SET` declares (it does not glob, so a missing file fails a test).
  The host's files stay pinned to `x86_64-unknown-linux-gnu`. A browser file obeys the deny-lists
  above, except that `polyoxide-core`, the HTTP stack itself, is not held to the socket-only one
  (`UNFENCED`); no file may build `WASM_NEVER` (`tokio-tungstenite` and `rustls`, the host-only
  transport), and each file named in `WASM_MUST_BUILD` must build the crates only the browser
  build reaches (`gloo-timers`, `send_wrapper`, `web-sys` and the like), so a target that resolved
  as the host's fails. A dependency that belongs to the host goes in its manifest's
  `cfg(not(target_arch = "wasm32"))` table, never into a file here. `--write` regenerates both sets
  and creates the browser set's files when absent; say why in the commit.
- `test_module_readmes.py` fails when a venue crate's `src/<product>/README.md` is not included
  exactly once under `cfg(all(doctest, feature = "<product>"))`, when such an include names no
  file, or when a module directory has no README and no entry in its `WITHOUT_README` (only
  `rtds`, whose guide is its `mod.rs` docs); an entry fails once its module has a README.
- `test_venue_gate.py` runs `scripts/venue_gate.py` (CAP-7) on the real tree, and on v0.39.0's
  core, which it must catch, so the job checks out every tag.
- `test_runtime_gate.py` runs `scripts/runtime_gate.py` on the real tree, which must stay clean,
  and on synthetic trees for each banned path, each skip and the opt-out.
- `test_claude_rules.py` holds `.claude/rules/` to its contract (CLAUDE.md, "Where the rules
  live").
- `test_gen_registry.py`, `test_spec_docs.py` (the endpoint counts CLAUDE.md and the spec indexes
  state) and the rest guard the scripts and the generated or counted text.

## The venue gate

`scripts/venue_gate.py` reads every file under `polyoxide-venue`, `-core`, `-ws`, `-rt` and
`-test-support`, case-insensitively, for:

- each venue id, as a substring;
- each product id, as an identifier segment (split at `_`, `-`, `.`, `::` and case boundaries, so
  `clob` in `clob_default` and `perps` in `PERPS_GENERAL`), less the generic words of
  `ci/venue-gate-skip.txt` (`data` and `events`);
- each `<venue>.<product>:` prefix;
- each venue's `identifiers` (Polymarket's `Poly-RateLimit`; Binance's `fapi`, `fstream` and
  `X-MBX`, its hosts and weight header);

all read from cargo metadata. A hit passes only through a `gate_exceptions` entry,
`{ path, term, reason }`, in the metadata of a crate of the hit's venue, and an entry that matches
nothing fails, so an exception never outlives what it excuses. Only a protected test name, or a
probe that measured one venue's host, takes an exception, in that venue's crate (today the ws
suites' `mod perps`, `mod binance` and their kin, excepted by `polyoxide-polymarket` and
`polyoxide-binance`).

## The runtime gate

`scripts/runtime_gate.py` (no arguments; exit 0 when clean, 1 on a hit, each hit printed as
`<path>:<line>: <banned path>`) holds every timer, monotonic clock read and spawn in the
`src/**/*.rs` of `polyoxide-venue`, `-core`, `-ws`, `-polymarket`, `-binance` and `-kalshi` to the
injected `polyoxide_rt::Runtime`, so a browser build can supply its own. It fails on
`tokio::spawn`, any `tokio::task::spawn*` (`spawn_blocking` and `spawn_local` too), any
`tokio::time` path, and `Instant::now` and `SystemTime::now`, called or passed as a function
value, and not clippy, whose `disallowed-methods` would also fire in every test and example,
which use tokio legitimately. It reads a `use` tree rooted at `tokio` leaf by leaf, so
`use tokio::{spawn, time::sleep};`, `use tokio::task;` and an `as` alias are hits. It skips an item
under a `cfg` that holds only in tests (`test`, or `all(..)` with `test` among its operands; an
`any(test, feature = "..")` is production code and is read), the file a `mod x;` under one
declares (inside an inline `mod y { .. }`, `y/x.rs`), `polyoxide-ws/src/test_server/` and every
`scripted.rs`; `polyoxide-rt` implements the runtime and is not scanned. A line ending
`// runtime-gate: <reason>` is exempt, as `live_unwraps.py`'s opt-out is: the two in tree are the
wall-clock reads, `UnixMillis::now` and `shared::auth::current_timestamp`, which are never
injected. The scripts job runs it through `test_runtime_gate.py`.

## wasm

`scripts/wasm_check.sh` runs `cargo check` and `cargo clippy -- -D warnings` for
`wasm32-unknown-unknown` over the supported set, crate by crate with its features named. It sets
`RUSTFLAGS='--cfg getrandom_backend="wasm_js"'` itself, replacing any inherited value, since an
inherited `RUSTFLAGS` (a host `target-cpu`) would override a config file's and break the target;
with `--target` cargo passes no `RUSTFLAGS` to build scripts, so the host cache is untouched. A
change to the supported set edits that script.

`scripts/wasm_smoke.sh` then runs `polyoxide-wasm-smoke`'s `tests/browser_smoke.rs` in headless
Chrome, inside a dedicated Worker, through `wasm-bindgen-test-runner`. It builds and starts the
native `wasm-smoke-server`, which binds `127.0.0.1:0` for mockito routes (every one answering CORS
headers: a 200 and two 429s, one exposing `Retry-After` and one not, on one server, and a slow
route on a server of its own, since mockito answers it by sleeping its server's one thread and
wasm-bindgen-test may run async tests concurrently), for one scripted socket on
`polyoxide-ws`'s test server and for a hand-written tokio responder whose one route sends its head
and the first bytes of its body at once and stalls the rest for 3 s (mockito writes a response
whole, so it cannot), and writes `POLYOXIDE_SMOKE_HTTP`, `POLYOXIDE_SMOKE_WS`,
`POLYOXIDE_SMOKE_SLOW` and `POLYOXIDE_SMOKE_STALL` to the file its one argument names. The script exports them, and the tests
read them with `env!`, so a build without them does not compile and the suite cannot pass empty.
The runner starts Chrome with the arguments in `polyoxide-wasm-smoke/webdriver.json`
(`--headless`, `--no-sandbox`, `--disable-gpu`, `--disable-dev-shm-usage`): without `--no-sandbox`
Chrome cannot start where unprivileged user namespaces are restricted, as on a hosted runner, and
the script refuses to run without the file.
It refuses a `wasm-bindgen-cli` whose version is not the `wasm-bindgen` one `Cargo.lock` pins,
since the runner rejects a module built by another. The suite covers `BrowserRuntime` (sleep,
`sleep_until`, `timeout`, `interval`, spawn, abort, `is_finished`, a monotonic `now`, a sleep past
`i32::MAX` ms), `FetchTransport` (a 200, a timeout, both 429s read through `ErrorResponse`) and
`WebSocketTransport` (a text echo, a close with its code, a refused handshake header and ping
frame). Three browser-only paths no host test reaches have a test each: a CLOB auth signature
through `Account::sign_clob_auth`, byte-equal to the host's py-clob-client golden vector, which
drives the wasm32 `SendWrapper` arm of `sign_digest` in `clob/core/eip712.rs` around alloy's
`?Send` `sign_hash` future (the smoke crate's browser dev-dependency on `polyoxide-polymarket`
with `clob`); a body read that outlives `FetchTransport`'s deadline, which comes back as
`TransportKind::Timeout` because the transport counts a failure at or after the deadline as one
(reqwest's wasm client reports that cut-short read as a decode error); and a window-quota wait
that paces on the runtime's clock, shown with a runtime running four times fast since quanta
reads the same `performance.now()` in a Worker and a real-clock test cannot tell the two apart.
Each test's failure was shown by breaking the one thing it pins, so a new test comes with
the same proof.

CI's job installs the wasm32 target and `wasm-bindgen-cli` at the locked version, and uses the
runner's Chrome and chromedriver. Locally, the flake provides chromium, chromedriver (Linux) and
the CLI; if the flake's CLI is not the locked version, move the lockfile to it with
`cargo update -p wasm-bindgen --precise <version>`. `scripts/runtime_gate.py` is not part of this
job: it runs in the scripts job.

## removals

`python3 scripts/api_removals.py check --baseline v0.40.0` runs cargo-semver-checks with
`--release-type patch` and fails on any reported removal that the listing for the workspace's
stage does not hold, printing each one's key to copy there.

- `check` and `release` read the listing `LISTINGS` names (`docs/s1-removals.md` in S1,
  `docs/s3-removals.md` in S3); `check` refuses stage S2. A key is
  `<crate> <lint id>: <item> (<file>)`; a deleted crate is `<crate> crate_missing`.
- cargo-semver-checks cannot see `#[doc(hidden)]` items or type aliases, so the job also imports
  each path the listing names under "Doc-hidden paths consumers import", in one scratch crate per
  crate and feature set, so no entry compiles through another's features. A story that removes
  one of those paths marks it `**Removed**` in the same commit.
- `docs/s3-migration.md` holds S3's consumer-impact notes, which the first S3 version-bump commit
  quotes in CHANGELOG.md.
- Final and no longer written: `docs/s1-removals.md`; S2's `docs/rename-manifest.md` (rows mapping
  every path, feature and CLI command that moved or went from `v0.39.0`) and `docs/rename-map.tsv`
  (the map `api_removals.py manifest --expand` wrote at the S2 release). No job runs `manifest`.
- cargo-semver-checks 0.51.0 and Rust 1.99.0 are pinned together in `ci.yml`, `release.yml` and
  `flake.nix`, because the tool reads only the rustdoc JSON some toolchains emit (1.93 to 1.99):
  upgrade both at once, in all three. `flake.nix` takes cargo-hack and cargo-semver-checks from
  nixpkgs, so check their versions in the shell after a `nix flake update`.
- `release.yml`'s `semver` job runs the same tool before anything publishes (`release.md`).

## test bodies (AD-12)

`python3 scripts/test_body_diff.py --baseline v0.40.0` (CI's one `S3_BASELINE`).

- **What it reads**, at the baseline and at the commit: every test-context item (fns, consts,
  statics, structs, enums, unions, type aliases, `extern` items, `macro_rules!` and item-level
  macro invocations, and the methods, consts and types of `impl` and `trait` blocks) in
  `tests/**`, `[[test]]` and `[[bench]]` targets, `[[example]]`s that set `test = true`, the files
  a test-context `mod` declares, modules and files whose `cfg` names `test` outside `not(...)`
  (`#[cfg(test)]`, `cfg(any(test, …))`, `#![cfg(test)]`), and anywhere under a test attribute.
  Also each `[[test]]` and `[[example]]` stanza (every field but `name` and `path`;
  `required-features` mapped through the manifest's `## Features` rows) and each non-Markdown
  file under a `tests/fixtures/`, by its blob.
- **Gates:** each item is compared with what gates it: the attributes of the `mod x;` that
  declares its file, the file's inner `cfg`/`cfg_attr`, and the attributes of each inline module
  around it, so a test switched off by one of those shows. A gate on an ancestor file module is not
  read: `lib.rs`'s `#[cfg(feature = "gamma")] pub mod gamma;` declares only `src/gamma/mod.rs`, so
  it gates the inline tests of the files under `src/gamma/` unseen.
- **Pairing:** by file (`git diff -M30%`, plus `[files]` in `ci/test-body-diff.toml`), then by a
  unique module path, kind and name.
- **Normalising** both sides before comparing attributes (doc comments aside), signature and body:
  comments and `use` statements dropped; a library's `crate::` as its name; the manifest's path
  rows, then the toml's `[paths]` (for a move into code that is not public, which no manifest row
  names), applied to the old side; a bare identifier renamed only at the head of a path;
  `include_str!`/`include_bytes!` targets as blob ids; the `[fixtures]` table on the old side (a
  baseline `fixtures!()` read as `fixtures!("<subdir>")` and `fixtures!("x")` as
  `fixtures!("<subdir>/x")` under a scope, for a crate whose fixtures moved under
  `tests/fixtures/<module>/`, applied before `[literals]`); the `[literals]` table on the old side;
  the `[gates]` table on the new side (under a path prefix, each feature's
  `cfg(feature = "<m>")`, or each whole predicate such as
  `any(feature = "clob", feature = "relay")`, dropped where it is a `cfg`'s whole predicate or an
  operand of its top-level `all(...)`, and each feature from `required-features`: for code that
  moved behind a module's features, such as a crate root that becomes `src/<m>/mod.rs`, or core's
  files in `shared`); whitespace collapsed; rustfmt's re-wrapping under longer paths ignored (a
  trailing comma before a closing bracket or a generic list's `>`, except in a one-element group
  that is not a call's, and braces around a lone expression after `=>` or a closure's `|…|`).
- **Fails** on a changed item unless a `[[drift]]` entry names a DRIFT row of
  `_bmad-output/specs/spec-venue-extensibility/divergences.md` and the diff's sha256; on a lost
  item unless a `[[retired]]` entry names it and an old path whose covering manifest row gives a
  reason; and on any `[files]`, `[fixtures]`, `[literals]`, `[paths]` or `[gates]` entry that
  pairs or rewrites nothing.
- **The 72 `[[drift]]` entries:** `mock_gamma`'s `get_many_fans_out_closed_true_and_false`
  (DRIFT R13); `spec_agreement_data_v2`'s `NOT_MODELLED`, `ROUTES`, `SPEC` and `agreement!#1`,
  which adopting a spec change moves (R14); `polyoxide-ws`'s scripted test server's
  `struct Recorder`, `fn handshake` and `fn serve`, changed to record each handshake's request
  headers (R15); `enum_wire_data_v2`'s
  `enums_that_also_appear_in_responses_spell_what_the_server_sends`, whose `ActivityType` set
  gained `MIGRATION` (R16) and then the four range-event types (#83), so it now cites R19;
  sixteen perps items that adopting its drift (#56) moved, among them
  `spec_agreement_perps`'s and `wire_agreement_perps`'s tables, four `mock_perps` tests,
  `perps::types`' enum test and the `leaderboard.json` and `leaderboard_account.json` fixtures
  (R17), seven of which adopting its builder-fee drift (#76) moved again and now cite R18;
  `live_perps`' `live_market_data_for_a_quoting_instrument`, which asserts `tickers` honours
  `instrument_id` (R20); and `perps::types`' enum test, moved again, and `spec_agreement_perps`'s
  `every_enum_matches_the_spec`, for `InstrumentCategory`'s undocumented `pre_ipo` (R21). The
  test bodies the wasm32 seams' stories changed cite R22: story 2's is `polymarket_limits`'
  `an_expired_cooldown_stops_delaying`, which advances virtual time past the quota's refill before
  its second acquire, since a window quota now keeps the runtime's clock and its pacing shows;
  story 3's, which name the request vocabulary (`Method`, `StatusCode`, `HeaderMap`, `header::*`) through `polyoxide_core::http`, are core's
  `mock_request` items `a_post_sends_its_body_serialised_once`,
  `a_request_s_costs_reach_the_throttle`, `a_request_parts_timeout_bounds_its_attempt` and
  `a_request_parts_timeout_outlasts_the_client_s`; binance's `usdm::error` `tests::response`,
  `failure_tags_usdm`'s `raw` and the `weight_probe` example's `used_after`; polymarket's
  `clob::error`'s, `data::error`'s, `gamma::error`'s, `perps::error`'s and `relay::error`'s
  `tests::response`, `data::v2::error`'s `tests::parts`, `shared::policy`'s `tests::retries` and
  `tests::retries_a_429`, and `send_loop`'s `a_429_with_no_retry_left_is_fail_with_its_hold`.
  The test bodies story 4 re-typed also cite R22, on the ground spec D12 gives (a row asserting an
  `ApiError::Network` case no transport can produce goes with the variant, its decode case
  re-pinned on `Response::json`): core's `config` test
  `http_builder_applies_every_knob_that_is_set`, `error`'s `ReqwestErrors`, `reqwest_errors` and
  `every_variant_classifies`, `health`'s `a_transport_failure_on_health_is_the_callers_error`
  and `mock_request`'s `a_request_parts_timeout_bounds_its_attempt` (story 3's entry, its hash
  replaced); polymarket's `send_loop`
  `a_transport_error_is_not_retried_and_skips_observe_and_decide`, `mock_relay`'s
  `relay_timeout_ms_bounds_a_plain_request` and `relay::error`'s `every_variant_classifies`;
  test-support's `failure_tags` `network_timeout` and `network_connect`; and Binance's `live_usdm`
  `raw` (Kalshi's live suite is newer than the baseline, so it takes no entry). Story 4 also fixes
  core's `mock_request` `retries_on_429_then_succeeds`, which mockito answered from its success
  mock first so it never retried: the 429 mock now goes first with `expect(1)`, the retry
  happens, and the entry cites R22 too. The test bodies story 5 changed cite R22 as well:
  `Kind::Unsupported`'s row (a sweep over a closed set of kinds gains the row for a variant the
  seams add), added to `every_other_kind`, to the kit's
  `every_transport_error_has_its_kind` and to `a_failure_that_replays_on_every_attempt_is_not_retried`;
  the kit's connect and test-server tests (`polyoxide-ws`'s `connect` tests and the test server's
  `a_refused_upgrade_carries_its_status_and_headers`, `a_stalled_handshake_never_completes` and
  `reject_handshake_fails_the_connection_attempt_recoverably`), whose `expect_err` became
  `.err().expect(..)` since a `BoxSocket` is not `Debug`; the CLOB's `a_dns_miss_is_a_network_failure`,
  whose body changed with its dial, and the three address tests that moved with that dial into the
  kit, gated to native (`connect_tests::addr`, `address_families_are_interleaved` and
  `single_family_lists_pass_through_unchanged`); and `live_sports`' `connect_raw`, returning a
  `BoxSocket`.
- `[gates]` also holds two entries for the kit's host-only code, `polyoxide-ws/src/test_server` and
  `polyoxide-ws/src/tls.rs`, each dropping `not(target_arch = "wasm32")`: the wasm32 seams gate
  that code off the browser build, which changes what gates its tests and nothing they assert.
- The manifest's rows still normalise the old side; their baseline is `v0.39.0`, so the script
  warns that they map paths from another tag, which is harmless at `v0.40.0`.
- git pairs only tracked files, so **stage a moved file before running it locally**. `--moves`
  prints each moved item's old `file:line name` and new `file:line`, for `docs/MUTANTS.md`.
  README and doc-comment doctests, and Python tests, are not compared.
