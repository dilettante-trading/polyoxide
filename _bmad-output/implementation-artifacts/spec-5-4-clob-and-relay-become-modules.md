---
title: 'Story 5.4: clob and relay become modules'
type: 'refactor'
created: '2026-10-13'
status: 'in-progress'
route: 'dispatch'
baseline_commit: '5117be372bdead89bac48ee067f0d34b9521e834'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-3-gamma-and-data-become-modules.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A Polymarket trader still depends on `polyoxide-clob` and `polyoxide-relay` beside `polyoxide-polymarket`. clob's `ws` feature and its `WebSocketError` break S2's naming convention, and nothing proves the signing stack (`alloy`) stays out of the credential-free builds.

**Approach:** Move both crates into `polyoxide-polymarket` as the modules `src/clob/` and `src/relay/`, as Story 5.3 moved gamma and data. Then:
- delete the old crates;
- rename the `ws` feature to `clob-ws` and `WebSocketError` to `ClobWsError`;
- stop `clob` implying `gamma`;
- set the crate's defaults to `clob`, `gamma` and `data`;
- prove per feature that `alloy` is built only with `clob` or `relay`.

The signing tests, golden vectors and kill-outcome tests move with empty body diffs.

**Decisions** (all decided overnight without the user, on the recommended option, 2026-10-13):
- **Module paths.** `polyoxide_clob::X` → `polyoxide_polymarket::clob::X` and `polyoxide_relay::X` → `polyoxide_polymarket::relay::X`. The internal files are kept (clob's `account/`, `api/`, `core/`, `ws/`, `authenticator.rs`, `utils.rs`; relay's flat files), and each old `lib.rs` becomes its module's `mod.rs`. `ClobWsError` lives at `clob::ws::ClobWsError`.
- **`WebSocketError`'s dead variants stay.** `ConnectionClosed` and `Authentication` are never constructed, but a protected unit test (`ws/error.rs` :197, :206) builds them, and removing them would change its body. S2 allows no DRIFT row for that, so the pruning is deferred to S3.
- **Features:**
  - `clob` adds clob's dependencies, `alloy` among them.
  - `clob-ws = ["clob", <clob's old ws deps>]`, with `polyoxide-ws` an optional dependency.
  - `relay` adds relay's dependencies and its alloy extras (`alloy?/provider-http`, `alloy?/rpc-types`).
  - `keychain` turns on core's `keychain` for whichever of clob and relay is enabled.
  - `default = ["clob", "gamma", "data"]`.
- **`clob` without `gamma`.** clob's gamma helpers keep their `cfg(feature = "gamma")`. Inside `src/clob/`, which `clob` gates, that `cfg` is in effect `all(clob, gamma)`, and a test body's gate stays as the baseline had it. `-F clob` alone builds, and so does `-F clob,gamma`.
- **Test target names:**
  - clob: `live_clob`, `live_clob_ws`, `live_clob_session_keys`, `mock_clob`, `error_classes_clob`, `headers_clob`, `query_setters_clob`;
  - relay: `live_relay`, `mock_relay`, `error_classes_relay`, `headers_relay`.
  - Each has `required-features`. `live_clob_session_keys` needs `clob`, `relay` and `gamma`.
- **Live suites:**
  - `clob` holds `live_clob` and `live_clob_ws`;
  - `clob-session-keys` holds `live_clob_session_keys` and keeps its 40-minute timeout and note;
  - `relay` holds `live_relay`.
  - Each keeps its own secrets.
- **Fixtures.** They move to `tests/fixtures/clob/session_keys/` and `tests/fixtures/relay/session_keys/`. `scripts/capture_session_key_vectors.py` becomes `scripts/capture_polymarket_session_keys.py`, writing to the new directories.
- **Examples.** clob's `test_order`, `ws_market` and `ws_user` move, the last two requiring `clob-ws`. relay's `get_nonce` and `redeem` move. relay's `hello`, a bare hello-world that uses no crate API, is dropped.
- **Dependents:**
  - The umbrella keeps its feature names: `clob`, `ws` (now meaning `polymarket/clob-ws`) and `keychain`. Its `pub use polyoxide_polymarket` also covers `clob`, and its prelude and `PolymarketError` take the new paths. Story 5.7 reshapes it.
  - The CLI and py repoint. py now builds clob together with gamma, so its `Clob` gains the gamma proxy lookup; this is accepted.
- **Scope.** The spec stays whole, about 3,000 tokens, because clob and relay share the signer, session-key and allowlist changes. It was approved with no human review, under the run's advance approval.

## Boundaries & Constraints

**Always:**
- **No shims.** `polyoxide-clob/` and `polyoxide-relay/` are deleted, along with their members and workspace dependencies. No alias or re-export keeps an old path or feature alive.
- **Gating.** Every module item and dependency is gated by the features that use it. `cargo hack check --each-feature` passes. The header pins `--no-default-features --features clob --test headers_clob` and the same for relay pass.
- **Tests move unchanged.** `test_body_diff.py --baseline v0.39.0` reports 0 diffs and 0 lost with every entry firing. That covers:
  - the EIP-712 and session-key golden vectors;
  - `classify_order_kill` and the FAK/FOK tests;
  - clob's 114 mock tests and relay's 55.
  - Rewrite HEAD paths so they normalise equal, for example `crate::clob::…`. Never edit an assertion or a test name.
- **The per-feature deny-list (NFR2).** `test_dep_allowlists.py` passes `--no-default-features`.
  - It keeps the socket-only deny-list for the socket crates' files.
  - For `polyoxide-polymarket` files, `alloy*` and `hmac` are denied unless the file's features include `clob` or `relay`, and `keyring` unless they include `keychain`.
  - Allowlists exist for `polyoxide-polymarket@gamma`, `@data`, `@clob` and `@relay`.
  - A positive check proves `@clob` and `@relay` contain `alloy`.
  - Regenerate with `--write`, and give the reason in the commit message.
- **The manifest.**
  - Rows:
    - `polyoxide_clob` → `polyoxide_polymarket::clob`, and `polyoxide_relay` → `polyoxide_polymarket::relay`;
    - `WebSocketError` → `ClobWsError` at every path it had;
    - the umbrella's clob paths;
    - features: `polyoxide-clob/{ws,gamma,keychain,default}` and `polyoxide-relay/{keychain,default}`, each to its new feature.
  - The doc-hidden `DynSigner` entries move to their new paths and features.
  - `manifest --strict` resolves every entry.
- **Registration moves with the code.** Add `clob` and `relay` to products, add `clob`, `relay` and `session-keys` to mirrors, add the live entries and update the readme line. Then run `gen_registry.py --write`.
- **Module READMEs.** `src/clob/README.md` and `src/relay/README.md` are gated doctests, linked from the crate README. `test_module_readmes.py` passes.
- **AD-21.** Every CLAUDE.md rule naming a moved path, test file, script, the `ws` feature or `WebSocketError` is rewritten in its own section.
- **CI-script tests.** Update their pins, and never weaken anti-vacuity.

**Never:**
- touch perps, rtds or sports beyond their doc text that names the old crates;
- remove the dead `WebSocketError` variants;
- add `[[drift]]` entries;
- create `tombstones/`;
- rename `keychain` or `specta`;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| clob without gamma | `cargo check -p polyoxide-polymarket --no-default-features --features clob` | builds; `ClobBuilder` has no `gamma` setter | N/A |
| Credential-free feature | `cargo tree -p polyoxide-polymarket --no-default-features --features gamma` | no `alloy*`, `hmac` or `keyring`; the allowlist test passes | N/A |
| alloy leaks | a change that makes `@data` build alloy | `test_dep_allowlists.py` fails, naming the crate and the rule | N/A |
| Renamed socket error | a baseline test naming `WebSocketError::MembershipClosed` | pairs with `ClobWsError::MembershipClosed`, 0 diffs | N/A |
| Golden vectors | `order_vectors.json` at its new path | the EIP-712 tests pass against the same blob | N/A |

</frozen-after-approval>

## Code Map

- **clob** (`polyoxide-clob/`):
  - **Cargo.toml.** `default = ["gamma"]`; `ws = [dep:polyoxide-ws, dep:futures-util, tokio/net, tokio/macros, tokio/sync]`; `keychain = [polyoxide-core/keychain]`.
    - Dependencies: alloy 1.1.2 `[network, signer-local, signers, sol-types]`, hex, rand 0.9.2 `[std]`, rust_decimal, urlencoding, url, `tokio[time]` and tracing.
    - Dev-dependencies: the versioned `polyoxide-relay`, dotenvy, rust_decimal_macros, tracing-subscriber, futures-util and `polyoxide-ws[test-server]`.
    - `[[test]] live_ws`.
    - Metadata `live.{live_api, live_ws, live_session_keys}` with secrets.
  - **lib.rs.** `#[cfg(feature = "ws")] pub mod ws` at :146; re-exports at :149-195; keychain re-exports at :157; README doctest at :133-135; `const _` at :200-206.
  - **Gamma gates:**
    - client.rs :27, :62, :553, :581, :816, :829, :891, :918, :936;
    - error.rs :24 (`ClobError::Gamma`), :199, :213, :225, and tests :254, :259, :285, :561;
    - mock_api.rs :1649 and :4445.
    - `tests/live_api.rs:19` and `live_session_keys.rs:36` import `Gamma` ungated, so their targets require `gamma`.
  - **ws.** `ws/error.rs:5` expands `impl_ws_classification!`, and `:8-52` holds `WebSocketError`. Because `::polyoxide_ws::` is named, `polyoxide-ws` must be a direct optional dependency.
  - **Fixtures.** `tests/fixtures/session_keys/{order_vectors,clob_auth}.json` and PROVENANCE.md, loaded by `include_str!` at account/mod.rs:728-729 and core/eip712.rs:1071 and :1340.
  - **Tests:** error_classes 3, headers 1, query_setters 1, mock_api 114, live_api ~45, live_session_keys 1 (uses `polyoxide_relay` at :37 and :140), live_ws 2.
- **relay** (`polyoxide-relay/`):
  - **Cargo.toml.** alloy adds `provider-http` and `rpc-types`; `keychain`; metadata `live.live_api` with secrets.
  - **src.** Private `account`, `client`, `config`, `error` and `types`; public `deposit_wallet`, `session_signers` and `wallet`. Re-exports at lib.rs; keychain at :156; README doctest at :118-120; `const _` at :164-167.
  - **Fixtures.** `tests/fixtures/session_keys/relay_vectors.json`, loaded by `include_str!` at deposit_wallet.rs:245, wallet.rs:178, session_signers.rs:289-290, client.rs:2076 and :2315, and tests/mock_api.rs:450.
  - **Tests:** error_classes 2, headers 1, mock_api 55, live_api 7.
  - Both crates export `KEYCHAIN_SERVICE`, `save_private_key_to_keychain` and `DynSigner`, each in its own module. Never re-export them at the root.
- **Dependents:**
  - **Root Cargo.toml.** Members at :5 and :13, workspace deps at :31 and :34.
  - **The umbrella:**
    - Cargo :15-18, :23-24, :28;
    - lib.rs :85-86, :98-99, prelude :107-113, `PolymarketError::Clob` :144-146, Classify arms :173, :187, :201, test :360-362;
    - README :22, :35, :38, :43, :44 and the doctest :124-131.
  - **cli.** Cargo :15, :27, :29. 22 `polyoxide_clob` uses, in ws/market.rs, ws/user.rs, credentials/mod.rs and clob/prices/*. `polyoxide_relay::KEYCHAIN_SERVICE` at credentials/mod.rs :151, :219, :262. Tests: prices_fetch.rs:7, prices_download.rs:3, live_api.rs:8.
  - **py.** Cargo:14 (`default-features = false`). 25 uses: types/clob.rs, clients/clob.rs, error.rs:43.
  - **Text only:** rtds/src/lib.rs:8. Leave sports' README migration table and its PROVENANCE, which are historical.
  - The polymarket crate's own docs: README:14; src/gamma/types.rs :11, :201; src/shared/session_signer.rs:3.
- **CI:**
  - `ci.yml` header pins :117-118 and the specta step :124-127. `test_ci_workflow` derives expected lines from metadata, so check what it demands for clob and relay.
  - nightly-behavioral.yml's hand-written comment :40-47.
- **`.github/scripts/tests/`:**
  - `test_dep_allowlists.py`: the command at :80-89, `parse_name` :57, the deny lists :43-46, applied at :104-107 and :139-141, the synthetic deny test :182-186, the existence test :124-128.
  - `test_classify_coverage.py`: :231-232 rows, :273 ws-sweep row → (`polyoxide-polymarket`, `ClobWsError`).
  - `test_publish_order.py:815`: a real order assertion naming `polyoxide-clob`.
  - `test_live_registry.py`: live `features` must match `required-features`.
  - `test_mutants_ledger.py:75-87`.
  - `test_module_readmes.py`.
- **`scripts/`:**
  - `live_unwraps.baseline.json:8-10, :14`;
  - `capture_session_key_vectors.py` :22, :29, :150-153 and :680-692, plus both PROVENANCE.md files (lines 4, 26-27);
  - `ci/test-body-diff.toml` (its :32-33 comment already sketches `"ws" = "clob-ws"`).
- **The manifest.** Doc-hidden rows at :122-123 (`polyoxide_clob::{,account::}DynSigner`) and :126 (`polyoxide_relay::DynSigner`); `:35` already uses the `ConnectionClosed` example.
- **docs/MUTANTS.md:** rows (e) :106, "(e), case" :107 and "(e), only a 400" :108, and the notes at :17 and :120. They cite clob error.rs :99, :102, :129, :348, :360, :368, :381, :392, :423, :462 and mock_api.rs :3366, :3400, :3464.
- **CLAUDE.md** hand-written rules:
  - :26, :32, :35, :62 (example commands);
  - :50 (CI jobs, the deny-list wording);
  - :94;
  - :159, :167, :173-174 (auth section paths);
  - :180, :182 (errors);
  - :347;
  - :450;
  - :466-471 (session keys);
  - :476, :480;
  - :528;
  - :541, :543 (WebSocket section).
  - Generated regions are regenerated.
- **Other docs:** README.md :44 and :138; `docs/specs/session-keys/README.md` :30-31, :76, :78, :171, :175, :177, :189; `docs/specs/session-keys/OBSERVED.md:10`; `docs/specs/gamma/OBSERVED.md:140`.

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps, never push, with messages ending `(Story 5.4)` and the attribution lines:

```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01BjnNeirn7DAPvQNrs5khCr
```

Record the per-binary test counts first, and stage moved files before running the body diff. Build with `CARGO_INCREMENTAL=0`. A rustc killed with signal 15 or exit 254 is earlyoom; re-run it. Run cargo-hack alone, and never alongside `test_dep_allowlists`.

**Execution:**
- [ ] `polyoxide-polymarket/`:
  - move src, tests, examples and fixtures per Decisions;
  - add features, deps and dev-deps (gated) with the new defaults;
  - add the stanzas, metadata, module READMEs and the gated `const _`;
  - update the crate README and lib docs.
- [ ] Rename `WebSocketError` → `ClobWsError` everywhere.
- [ ] Delete `polyoxide-clob/` and `polyoxide-relay/`; update the root `Cargo.toml` and `Cargo.lock`.
- [ ] Repoint the umbrella (and its README doctest), cli and py; update the rtds doc line.
- [ ] `test_dep_allowlists.py` and `ci/dep-allowlists/`: the per-feature rules, the new files and the positive alloy check, with synthetic tests for the conditional rule.
- [ ] `ci.yml` pins; nightly-behavioral's comment; the other CI-script tests; `live_unwraps.baseline.json`.
- [ ] `scripts/capture_polymarket_session_keys.py` (renamed) and the PROVENANCE text.
- [ ] `ci/test-body-diff.toml` entries, each firing; `docs/rename-manifest.md` rows, then `api_removals.py manifest --write`.
- [ ] `docs/MUTANTS.md`, re-cited and re-proved.
- [ ] CLAUDE.md rules, other docs, then `gen_registry.py --write`.

**Acceptance Criteria:**
- Given the moved tree, when clippy (`-D warnings`), `cargo test --all-features --workspace`, the doctests and `RUSTDOCFLAGS=-D warnings cargo doc` run, then all pass, and the per-binary counts sum to the baseline's.
- Given the same tree, when `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` and every minimal header pin run, then both pass.
- Given the same tree, when `test_body_diff.py --baseline v0.39.0` runs, then it reports 0 diffs and 0 lost. And `api_removals.py manifest --baseline v0.39.0 --strict` exits 0.
- Given the scripts job, when its pytest runs, then it is green, including the per-feature allowlist rules. And `polyoxide-py`'s pytest passes after `uv sync --reinstall-package polyoxide`.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass, counts as at baseline
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run alone)
- `cargo test -p polyoxide-polymarket --no-default-features --features <m> --test headers_<m>` for gamma, data, clob and relay -- expected: pass
- `cargo check -p polyoxide-polymarket --no-default-features --features clob` -- expected: pass
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
- `python3 scripts/gen_registry.py --check` -- expected: exit 0
- `cd polyoxide-py && uv sync --reinstall-package polyoxide && uv run pytest tests/ -q` -- expected: pass
