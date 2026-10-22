---
paths:
  - "**/tests/**"
  - "polyoxide-test-support/**"
  - "scripts/capture_*.py"
---

# Testing conventions

- Suites are named `<kind>_<module>.rs` (`mock_clob.rs`, `live_usdm_ws.rs`, `venue_perps.rs`).
- **Mock HTTP:** `mockito` (a workspace dev-dependency). Each HTTP module has a
  `tests/mock_<module>.rs`: `polyoxide-polymarket`'s `mock_clob`, `mock_gamma`, `mock_data` (v2 in
  `mock_data_v2.rs` beside it), `mock_relay` and `mock_perps`; Binance's `mock_usdm`; Kalshi's
  `mock_predictions`, whose later suites are files of their own named `mock_predictions_<what>`.
  Each has helpers like `test_public_clob(server)` that point clients at the mock server's URL.
- The read-only modules build with `Gamma::builder().build()` and `DataApi::builder().build()`;
  CLOB tests use `Clob::public()` for unauthenticated endpoints.
- **Sockets:** the socket modules `rtds` and `sports` have no mock suite; their offline suites
  (`supervision_rtds`, `supervision_sports`, `bare_sports`), like every other socket suite, run on
  the kit's scripted server (`sockets.md`).
- **Live:** `tests/live_<module>[_<suite>].rs`, `#[ignore]`d, run with `-- --ignored`
  (`live-tests.md`).

## `polyoxide-test-support`

The agreement, fixture and soak helpers exist once, here (`publish = false`, a path-only
dev-dependency of the crates whose tests use it; it names no venue):

- `fixtures!("sub")` reads `tests/fixtures/sub/` of the crate that expands it (a macro, because
  `env!("CARGO_MANIFEST_DIR")` inside test-support names test-support).
- `agreement`: `key_paths`; `assert_values_agree` (`Arrays::Zip` for perps REST and Binance, whose
  kline drops an element on purpose; `Arrays::SameLength` for perps ws); Binance's
  `compare`/`compare_values`; the allow-list `Ledger` (`excuse`, perps' `never_on_wire`, `stale`)
  over `(fixture, path, reason)` or `(path, reason)` tables; gamma's dotted walker as
  `agreement::dotted`.
- `openapi`: the schema synthesiser and `check` (perps' `$ref`-nullable, enum, example and
  `OBSERVED_EXTRA` handling, plus data's refusal of two `allOf` arms).
- `query` (feature): `keys_sent` reads a builder's query keys off a mock server; `pairs_sent` its
  ordered key/value pairs (it does not need the body to decode), with `Case` and `assert_cases`
  running a table of builder calls over it for each `tests/query_setters*.rs`; `headers_sent`
  the full header set one request carries, for each `tests/headers*.rs`.
- `minute` keeps a test clear of a UTC minute boundary; `registration!()` reads the ids the calling
  crate declares in its `[package.metadata.polyoxide]`.
- `soak` (feature): the harnesses' `Pacer`, `percentile`, `parse_stages`, `parse_routes`, the
  `observe` throttle layer (the `WARN` under `polyoxide_core` that `tests/observe.rs` proves
  against core's client), and the two verdict rulebooks, `soak::verdict::tolerant` (data's) and
  `soak::verdict::strict` (perps'), which are different policies and stay apart.
- Failure tags and the credential loaders: `live-tests.md`.
- Each suite keeps a thin local function with its old name that states its own allow-lists and
  venue facts, so test names and bodies did not move. Binance's live and agreement targets alias
  the module as `use polyoxide_test_support::agreement as common;`.

## Tests assert the counterparty's value

Pin what the other side computes or sends, not a shape: a signing or encoding test against a
reference implementation's golden vectors (py-clob-client's for the EIP-712 type strings, py-sdk's
for session keys), never a self-computed value; an agreement test against captured wire fixtures;
a rate-limit test against the effective quota. A new rule or fix comes with a test shown to fail
without it.

## Capture scripts

`scripts/capture_*.py` share `scripts/capture_common.py`: `get` (it takes a `headers(method, url)`
function called per request, so signing stays in each script; a non-JSON error body is returned,
not raised), `require_ok`, `write_json`, `write_raw`, `stamp`, `provenance_table`,
`write_provenance` and the async `ws_session`, which imports `websockets` lazily. Run a capture
script as a file (`python3 scripts/capture_x.py` or `uv run scripts/capture_x.py`), never
`python3 -I`, which keeps `scripts/` off `sys.path`. `scripts/capture_binance_usdm.py` is a PEP 723
`uv run` script.
