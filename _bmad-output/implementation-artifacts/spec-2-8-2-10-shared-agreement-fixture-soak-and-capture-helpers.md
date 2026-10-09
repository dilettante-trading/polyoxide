---
title: 'Stories 2.8, 2.9 and 2.10: Shared agreement, fixture, soak and capture helpers'
type: 'refactor'
created: '2026-10-08'
status: 'ready-for-dev'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-2-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Inventory rows T1–T9 are copied across data, perps, gamma, binance and the capture scripts: key paths, value comparison, allow-lists, the OpenAPI synthesiser, query-key checks, fixture loaders, minute waits, the soak harness, and the capture scripts' HTTP and WebSocket clients.

**Approach:**
- Move each helper into `polyoxide-test-support`, as the superset of its copies, and into a new `scripts/capture_common.py`.
- Every caller keeps a thin local shim with the old name and signature, so test bodies, names, assertions and counts are unchanged (NFR7, AD-12).

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **One bundle, one review layer.** Work row by row: T6, T1, T2, T3, T4, T5, T7, T8, T9.
- **T2 `assert_values_agree(what, path, wire, emitted, Arrays)`.** `Arrays::Zip` keeps today's behaviour for perps REST and binance; binance's REST kline drops an element on purpose. `Arrays::SameLength` is for perps ws. Data and gamma keep comparing no values.
- **T3: one excuse ledger, plus gamma's dotted walker kept as it is.**
  - A `trait Excuse` covers both `(fixture, path, reason)` triples and `(path, reason)` pairs, so every const table stays as written.
  - `Ledger<E>` provides `excuse`, perps' `never_on_wire` prefix rule, and `stale()`.
  - Each caller keeps its own stale-check form: a set (data, perps), a count (binance ws), or **none (gamma)**. Giving gamma a stale check would add a test.
  - `agreement::dotted::check` is gamma's walker verbatim, array-length assertion included. `unmodelled_top_level` serves its direction-2-only test.
- **T4 synthesiser.** It is the perps fork (`$ref`-nullable, `enum`, `example`, inline objects, `OBSERVED_EXTRA`) plus data's "declared by two allOf arms" assertion. A Python re-implementation showed it yields identical objects on data's 42 schemas, and that perps' 30 have no duplicate allOf keys.
- **T6 fixtures.** A `fixtures!("sub")` macro expands `env!("CARGO_MANIFEST_DIR")` at the call site. It has to be a macro: `env!` written inside test-support resolves to test-support's own directory. py's and sports' in-`src` loaders, and gamma's `include_str!`, stay as they are.
- **T7.** `minute::wait_needed(window, land_at) -> Option<Duration>` (pure) plus an async `wait_unless_within`. Binance `mock_api.rs` uses `(0..=57_000, 100)`, and `weight_probe.rs` uses `(3_000..=20_000, 3_000)`.
- **T8, the soak harness.** `soak::{Pacer, percentile, parse_stages, parse_routes}`, plus `soak::observe` (data's observer as the superset, adding perps' per-path counts).
  - **The verdict rulebooks are two different policies, not copies of one.** Both move **verbatim**, under neutral names, with their tests unchanged: data's as `soak::verdict::tolerant` and perps' as `soak::verdict::strict`. Unifying them would rewrite perps' tests (NFR7).
  - **Route parsing:** perps adopting the generic `parse_routes` gains duplicate refusal, a strictly additive check; record it.
  - Data's `examples/common/mod.rs` is deleted, and its three data-specific constants are inlined into the `closed_positions_*` examples.
- **T9.** `scripts/capture_common.py` holds `get(url, *, params, headers, pause, ua, timeout) -> Reply`, where `headers(method, url)` is called on every attempt and a non-JSON error body is tolerated. It also holds `require_ok`, `write_json`, `write_raw`, `provenance_table`, `stamp`, `write_provenance`, and an async `ws_session(url, *, headers, max_size, ping_interval, create_connection)` that imports `websockets` and `certifi` lazily.
  - **Binance's capture script** drops its hand-rolled RFC 6455 client for `ws_session`, and so becomes a PEP 723 `uv run` script, no longer `python3 -I`.
  - `docs/specs/binance/probes/wsprobe.py` is out of scope (deferred).
- **Dependencies.** test-support gains `serde`, `serde_json`, `tokio` (with `time`) and `tracing`. Optional features `query` (mockito, url) and `soak` (tracing-subscriber) keep those dependencies out of crates that do not need them.

## Boundaries & Constraints

**Always:**
- **NFR7:** report per-file test counts before and after, from the tables in the Code Map. Every moved test keeps its name and assertions. Test-support's own new unit tests are reported separately.
- **`#[track_caller]` on every asserting helper,** so a failure still points at the test.
- **test-support stays venue-free:** no `polymarket`, `binance`, `clob`, `relay` or `kalshi` in code or docs (the S2 CAP-7 gate). Facts specific to one venue stay in the caller's shim.
- **`#![warn(missing_docs)]` is on,** so document every new `pub` item.
- **The mutants ledger:** moving binance's minute wait shifts the lines `docs/MUTANTS.md` and `test_mutants_ledger.py`'s `SNIPPETS` pin (binance `mock_api.rs` :626 and :662). Update both, and re-prove those mutants.
- **Prove the WARN detection:** add a test-support test that drives core's `HttpClient` against mockito, returning 429 then 200, and asserts the observer counted it at `WARN` under the `polyoxide_core` target prefix. Use `with_default`, never `.init()`.
- **Disk:** use `CARGO_INCREMENTAL=0` and `-j 4`. Do not run `api_removals.py`.
- **After D2 (8ca6620):** binance `tests/common/mod.rs` carries four `// live-unwraps:` opt-outs, which are decode and wire assertions and so real by design. Once it is deleted, remove its `opted_out` entry from `scripts/live_unwraps.baseline.json` by hand. The moved helpers keep panicking untagged, so they still file as `real`. The Code Map's line numbers are as of 43abff0; D2 shifted binance `common/mod.rs` and the live targets, so locate each site by name.

**Never:**
- Change a moved assertion.
- Add value or length checks where a caller had none, or give gamma a stale check.
- Unify the two verdict rulebooks.
- Touch a `tests/live_*.rs` beyond binance's `mod common;` switch.
- Put a personal name or email address in any request. The capture User-Agent stays `polyoxide-fixture-capture`.

</frozen-after-approval>

## Code Map

- **T1 `key_paths`:** data `tests/v2_wire_agreement.rs:158`; perps `tests/wire_agreement.rs:52` and `tests/ws_wire_agreement.rs:33`; binance `tests/common/mod.rs:14`.
- **T2:** perps `wire_agreement.rs:73` and binance `common/mod.rs:35` take `(what, path, wire, emitted)` and zip arrays. perps `ws_wire_agreement.rs:49` takes `(wire, emitted, path)` and asserts length.
- **T3:**
  - data `v2_wire_agreement.rs:31-290`: triples, problems collected, stale check by set at :281;
  - perps `wire_agreement.rs:27-181`: triples plus the `NEVER_ON_WIRE` prefix rule, stale check by set difference at :171;
  - binance `ws_wire_agreement.rs:11-45`: `IGNORED` only, stale check by count;
  - gamma `wire_agreement.rs:74-309`: dotted pairs, `check` at :261 with an array-length assertion at :294, the top-level direction-2-only check at :499, and no stale check.
- **T4:** data `v2_spec_agreement.rs:45-203` (with the allOf-duplicate assertion at :127); perps `spec_agreement.rs:24-254` (`ref_name`, `is_nullable`, `enum`, `example`, `OBSERVED_EXTRA` with its stale check at :188).
- **T5:** data `v2_spec_agreement.rs:345` (asserts the operation exists, :661); perps `spec_agreement.rs:399` (defaults to empty).
- **T6:**
  - data `v2_spec_agreement.rs:350`, `v2_wire_agreement.rs:176`;
  - perps `spec_agreement.rs:404`, `wire_agreement.rs:101`, `ws_wire_agreement.rs:23` (returns `(String, Value)`);
  - binance `wire_agreement.rs:27` (plus the closure at :62), `mock_api.rs:26`;
  - CLI `tests/data_v2.rs:26` (reads `../polyoxide-data/...`).
- **T7:** binance `tests/mock_api.rs:58`; `examples/weight_probe.rs:110`.
- **T8:**
  - data `examples/common/mod.rs:155-242` (`Pacer` :213, observer :67), included by `#[path]` into `v2_soak/main.rs:59`, `closed_positions_soak.rs:42` and `closed_positions_burst_probe.rs:57`;
  - the WARN detection: `closed_positions_soak.rs:534`, `closed_positions_burst_probe.rs:497`;
  - data's verdict rulebook: `v2_soak/verdict.rs:35/153/247`;
  - perps `examples/info_soak.rs:167-356` (`Pacer` :288, observer :332, verdict :178/223/278, stages :598, routes :73);
  - core's WARN sites: `request.rs:160`, `client.rs:219`.
- **T9:** `scripts/capture_{v2,perps}_fixtures.py` (`get` at :30 and :28); `scripts/capture_binance_fixtures.py` (`get` :37, RFC 6455 client :63-109); `scripts/capture_perps_ws_fixtures.py`; `scripts/capture_sports_fixtures.py`; `scripts/capture_session_key_vectors.py` (PROVENANCE writers at :527 and :554).
- **Test counts today** (NFR7):

  | File | Tests |
  | --- | --- |
  | data `v2_spec_agreement` | 6 |
  | data `v2_wire_agreement` | 1 |
  | perps `spec_agreement` | 5 |
  | perps `wire_agreement` | 1 |
  | perps `ws_wire_agreement` | 3 |
  | gamma `wire_agreement` | 20 |
  | binance `wire_agreement` | 2 |
  | binance `ws_wire_agreement` | 2 |
  | binance `mock_api` | 25 |
  | binance `weight_probe` | 2 |
  | data `examples/common` | 10 each, in 3 examples |
  | data `v2_soak` | main 10, probes 10, verdict 21 |
  | data `closed_positions_soak` | 20 |
  | data `closed_positions_burst_probe` | 10 |
  | perps `info_soak` | 11 |
  | CLI `data_v2` | 24 |

## Tasks & Acceptance

**Execution:**
- [ ] `polyoxide-test-support/` -- new modules `fixtures`, `agreement` (with `excuse` and `dotted`), `openapi`, `query` (feature), `minute`, and `soak` (feature: `observe` and `verdict::{tolerant,strict}`). Each comes with its own unit tests and the WARN-detection test.
- [ ] data, perps, gamma, binance and CLI tests and examples -- switch to the shared helpers through local shims. Delete each copy, binance's `tests/common/mod.rs`, and data's `examples/common/mod.rs`.
- [ ] `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py` -- update binance's citations and re-prove its mutants.
- [ ] `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md` -- mark rows T1–T9 resolved, through `bmad-spec` update.
- [ ] `scripts/capture_common.py` -- new. All six capture scripts use it, and binance becomes PEP 723 with `websockets`. Its tests, in `.github/scripts/tests/test_capture_common.py`, use a local `http.server` and need no network; the `websockets` parts are skipped when the package is absent.
- [ ] Re-run each capture script once:
  - session keys must be byte-identical;
  - v2, perps, perps_ws (instrument 6) and binance must keep the same file list, formatting and key paths;
  - sports must emit frames covering the committed shapes.

  Record the outcomes, and restore any live-data changes to the committed fixtures unless a shape changed.
- [ ] CLAUDE.md -- one paragraph naming the shared helpers and `capture_common.py`. Update any sentence that points at a removed copy.

**Acceptance Criteria:**
- Given rows T1–T9, when the tree is searched, then no copy remains in data, perps, gamma, binance or the capture scripts.
- Given every moved suite, when it runs, then its names and counts equal the table, and it passes.
- Given core's `HttpClient` hitting a 429, when the shared observer watches, then it counts the WARN.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `cargo test -p polyoxide-test-support --all-features -j 4`, and `cargo test -p <data|perps|gamma|binance|polyoxide-cli> --all-features --tests --examples -j 4`, all with `CARGO_INCREMENTAL=0` -- expected: pass, with counts as in the table.
- `cargo clippy --workspace --all-targets --all-features -j 4 -- -D warnings` -- expected: clean.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass.
- `python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py` -- expected: exit 0.
