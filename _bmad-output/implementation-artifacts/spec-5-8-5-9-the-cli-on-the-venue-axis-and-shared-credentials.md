---
title: 'Stories 5.8 and 5.9: the CLI on the venue axis, and credentials over a shared trait'
type: 'refactor'
created: '2026-10-15'
status: 'ready-for-dev'
route: 'dispatch'
baseline_commit: '73a47f602e7e5baf76bf19e4a6a66287ab8d24f0'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-7-the-multi-venue-umbrella.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The CLI groups its commands by transport, not venue. `ws binance` sits next to `ws prices --topic binance` (RTDS), and only `clob prices download` sits under its module. Five socket commands carry five `OutputFormat`s and two copies of the stream loop (C1, C2). market, user and prices still run an untestable ctrl-c + `select!` loop. Some list flags still lack `value_delimiter`. `polyoxide credentials` is Polymarket-only plumbing: its field lists are copied from the library, and it has no validation. A second venue would need all of it again.

**Approach:** Rebuild the tree as `polyoxide <venue> <module> <verb…>`, with `stream` as the verb for every socket, one shared runner and one shared `OutputFormat`. Add `StoredCredential` to core's keychain, with generic store, show and delete. Implement it for Polymarket's `clob` and `builder` kinds, and serve them through one generic `polyoxide <venue> credentials <store|show|delete> <kind>`.

**Decisions:**
- **The tree.** `completions` stays top-level, and the `ws` group and the top-level groups go, with no shims.

  | v0.39.0 | Now |
  |---|---|
  | `gamma …`, `data …`, `clob prices download` | under `polymarket` |
  | `ws market`, `ws user` | `polymarket clob stream market`, `polymarket clob stream user` |
  | `ws prices` | `polymarket rtds stream` |
  | `ws sports` | `polymarket sports stream` |
  | `ws binance` | `binance usdm stream` |
  | `credentials <verb> clob` | `polymarket credentials <verb> clob` |
  | `credentials <verb> relay` | `polymarket credentials <verb> builder` |

  Flags keep their names.
- **The runner (C2).** One `polyoxide_cli::commands::stream` module holds `OutputFormat`, the loop and `excerpt()`.
  - The loop handles count, deadline, a too-large timeout meaning no deadline, a closed reader ending quietly, and "The feed ended".
  - Each socket command keeps a `run_with(args, events, out, err)` that drives it. market, user and prices gain one, and lose ctrl-c, `select!` and the `ctrlc` dependency.
  - A stream error those three cannot skip now ends the run with an error. It used to print `Error:` and exit 0.
- **`OutputFormat`** (the user's choice, 2026-10-15). It is `{pretty, json}` on every stream.
  - `pretty` is one human-readable line per event, which is market and user's old `summary`.
  - `json` is one compact JSON object per line.
  - Indented JSON and `summary` are gone, so market and user's default output changes.
  - market's `format_summary` test retires.
- **C1.** The five stream copies become one `OutputFormat`. The dataset writer's `--format csv|jsonl|parquet` enum is a different concern and stays as it is. Renaming it would make the body diff rename every bare `OutputFormat` in the crate's baseline tests.
- **C3.** The shared module gets its own tests of the four runner behaviours. `tests/ws_sports.rs`, `tests/ws_binance.rs` and `tests/data_v2.rs` keep their names and assertions; only their `use` lines change.
- **List flags.** Every `Vec` argument uses `value_delimiter = ','`, positionals and `--filter`s included. The one exception is the hidden trailing `_ignored` of `data positions`. A clap-tree walk test enforces this, and each such argument has a parse test that passes a comma list.
- **`StoredCredential`, in `polyoxide_core::keychain`:**
  - `const SERVICE`, and `const FIELDS: &[Field]`, where `Field { name, secret }`.
  - `fn validate`, over the values to store.
  - Generic `store`, `show` and `delete` functions, with values carried as `Secret<String>`.
  - Implemented for `clob::Account` (`polyoxide-clob`: `private_key`, `api_key`, `api_secret`, `api_passphrase`, all secret) and for `relay::BuilderAccount` (`polyoxide-relay`: `private_key`, `api_key`, `api_secret` and `passphrase` secret, `relayer_api_key` secret, and `relayer_api_key_address` not secret).
  - Both libraries' `delete_from_keychain` delete through it, so each field list exists once.
- **Validation.** It runs before anything is written:
  - no empty or whitespace-only value;
  - `private_key` parses as a hex signing key;
  - `relayer_api_key` and `relayer_api_key_address` are given together, and the address parses.

  A refusal is a new `KeychainError::Invalid { service, field, reason }`, classed `InvalidRequest`.
- **The CLI's credentials command.** Its flags are generated from `FIELDS` (`api_passphrase` → `--api-passphrase`), so today's flags are kept. The verbs, flag generation, redaction and keyring calls live in the shared `commands::credentials` module. Polymarket's code lists its two kinds and nothing more.
  - `store` writes only the fields given, as today.
  - `show` prints each field as `present`, `not found` or `error: …`. A non-secret field that is present prints its value.
- **Retired tests.**
  - The tests of the old tree retire through `[[retired]]` with manifest reason rows, each replaced by a test of the new path:
    - main.rs: the 13 that name `gamma`, `data`, `clob` or `ws`;
    - `credentials/mod.rs`: its 8.
  - A reason row covers each removed root re-export and the `ws` and old credentials types. Every other moved CLI path gets a rename row.
- **The manifest covers the CLI tree** (the user's choice, 2026-10-15; G04's CLI half).
  - `docs/rename-manifest.md` gains a hand-written `## Commands` section. It maps each v0.39.0 command to its new command, or gives a reason, as in the tree table above, and includes the `relay` → `builder` kind and the `--format summary` removal.
  - A CLI test reads that section and fails unless every new command it names parses as a subcommand path.
  - G04's Python half stays with 5.10.
- **`Secret` on the public credential fields is re-deferred to S3** (the user's choice, 2026-10-15).
  - `Credentials`, `BuilderConfig` and `ApiCredentials` keep their `String` fields.
  - `Secret` holds secret values only inside the `StoredCredential` plumbing.
  - Changing the fields would change about 15 protected test bodies, and S2 allows no DRIFT row but R6.
  - DW:93's entry is re-filed for S3, with this reason.
- **Spec size** (the user's choice, 2026-10-15). 5.8 and 5.9 stay one spec, at about 3,800 tokens.

## Boundaries & Constraints

**Always:**
- No shims.
- `test_body_diff.py --baseline v0.39.0` reports 0 diffs and 0 lost tests, beyond its `[[retired]]` entries, and adds no `[[drift]]` entry.
- `api_removals.py manifest --strict` exits 0.
- Every v0.39.0 `polyoxide_cli` path has a row.
- Every moved test keeps its name and assertions.
- Every credentials kind and verb has a parse test.
- `show` never prints a secret value. A test renders a secret field that is present and finds no trace of its value.
- The services `polyoxide-clob` and `polyoxide-relay`, and their field names, are unchanged. Values stored by v0.39.0 load as before.
- AD-21: CLAUDE.md's CLI paragraphs (the `ws` group, `ws binance`, `clob prices download`, the `data` group, the clap `Vec` note and the keychain line) are rewritten in their own sections. The generated regions are regenerated.

**Never:**
- touch Python, the umbrella, the venue crates' features or modules, or a venue's socket behaviour;
- weaken a CI-script assertion;
- edit `docs/ARCHITECTURE.md`, `docs/s1-*.md` or the CHANGELOG;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Old path | `polyoxide ws sports` | clap error: unrecognized subcommand | exit 2 |
| Comma list | `polymarket clob stream market a,b --filter book,price` | two asset ids, two filters | N/A |
| Count with filter | market `-n 2 --filter book`, events price, book, book | two books printed, then "Reached 2 update(s)" | N/A |
| Bad store | `polymarket credentials store builder --relayer-api-key k` | nothing written, names the missing address | `KeychainError::Invalid`, exit 1 |
| Show | builder with every field present | the address in clear; every secret field as `present` | N/A |

</frozen-after-approval>

## Code Map

- **The CLI (`polyoxide-cli/`):**
  - **`src/main.rs`.** `Cli` and `Commands` at :9-44, and the dispatch at :52-63. Its 21 tests are at :86-257: 13 retire, and the `completions`, `no_subcommand` and `invalid_subcommand` tests stay as they are.
  - **`src/commands/mod.rs`.** It re-exports the groups at the crate root. The root re-exports go, and `stream`, `polymarket` and `binance` are added.
  - **`src/commands/ws/`:**
    - `sports.rs` has the C2 loop at :75-153 and `excerpt` at :270. Its `Filter`, `print_update` and `fit` stay, and the excerpt test moves to `stream`.
    - `binance.rs` has the loop at :174-243 and `excerpt` at :330.
    - `market.rs` and `user.rs` have `select!` loops and `print_*_summary`, which prints with `println!`. Turn it into a value written to `out`, keeping each test's call as written: `print_market_summary(msg)` is called by a test.
    - `user.rs` keeps `get_credentials` and `CredentialSource`.
    - `prices.rs` has a `--symbol` without a delimiter at :48.
  - **`src/commands/{gamma,data,clob}/`** move under `polymarket/` unchanged. `clob/prices/download.rs:52` `--token-id` gains its delimiter, and `clob/prices/types.rs` `OutputFormat` stays.
  - **`src/commands/credentials/mod.rs`** (417 lines). The old enums and duplicated field lists are replaced by the generic module, and its 8 tests retire.
  - **`src/commands/common/parsing.rs`.** `parse_list_entry`, which every list flag uses, and `parse_duration`.
  - **`tests/`.** `ws_sports.rs` and `ws_binance.rs` (C3), `data_v2.rs`, `prices_*.rs` and `live_api.rs`. Only their `use` paths change; `prices_download.rs:21` names a path inline, which a manifest row normalises.
  - **`Cargo.toml`.** Drop `ctrlc`. `keychain` stays, and dev-deps add `rtds`/`test-server` if the new market, user and prices tests use rtds' fixtures.
  - **`README.md`.** About 92 invocation lines to rewrite: 27-29, 45-116, 120-245, 248-333, 336-372 and 375.
- **Core.** `polyoxide-core/src/keychain.rs` holds `get`, `set`, `delete` and `KeychainError`, all `InvalidRequest`. Add `Field`, `StoredCredential`, the generic functions and `Invalid`. `polyoxide_venue::Secret` (`new`, `expose`) is all there is.
- **polyoxide-polymarket:**
  - `src/clob/account/mod.rs`: `KEYCHAIN_SERVICE` at :44, `from_keychain` at :313 and `delete_from_keychain` at :374.
  - `src/relay/account.rs`: :17, `from_keychain` at :121, `delete_from_keychain` at :182, and `RelayerApiKeyConfig::new`'s non-empty rule at `config.rs:188`.
  - Both are gated `keychain`.
- **Gates:**
  - **`ci/test-body-diff.toml`.** Add `[[retired]]` entries, in the format at :80-86. The baseline file paths are `polyoxide-cli/src/main.rs` and `polyoxide-cli/src/commands/credentials/mod.rs`. Add `[paths]` only for private paths a test body names inline, and `[files]` only if git does not pair a moved file.
  - **`docs/rename-manifest.md`.** Rows for `polyoxide_cli::commands::{clob,data,gamma}` → `…::polymarket::…`, `ws::sports` → `polymarket::sports`, `ws::binance` → `binance::usdm`, and each `OutputFormat` → `stream::OutputFormat`. Reason rows for `ws`, the root re-exports and the old credentials command. Then run `--write`. A row whose last segment changes renames bare heads crate-wide (`_renames`), so keep the arg-type names `SportsArgs`, `BinanceArgs`, `MarketArgs`, `UserArgs` and `PricesArgs`, and every `run_with`.
- **Docs:**
  - CLAUDE.md :89-125 and :250, plus :7 and :402 if they read wrongly;
  - root `README.md` :55-69;
  - `docs/specs/data-v2/INDEX.md:98`;
  - deferred-work's G04 and DW:93 entries.

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps and never push. Each message ends `(Stories 5.8 and 5.9)`, followed by:
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
`Claude-Session: https://claude.ai/code/session_01ECNn4RgL4d8WZCGXgeUi2T`

Also:
- Record the per-binary test counts first.
- Use `CARGO_INCREMENTAL=0`. A signal 15 or an exit of 254 is earlyoom, so re-run.
- Stage moved files before running the body diff.

**Execution:**
- [ ] `polyoxide-cli/src/commands/stream.rs`: `OutputFormat`, the loop and `excerpt`, plus the four C3 tests over a scripted feed.
- [ ] `polyoxide-cli/src/commands/{polymarket,binance}/`: move the files and rebuild the tree. Port sports and Binance to `stream`. Give market, user and prices a `run_with` on `stream`. Add the list delimiters. Rewrite `main.rs`, with replacement tests and the clap-walk delimiter test. Add `tests/stream_polymarket.rs`, which drives market, user and prices through `run_with`, and drop `ctrlc`.
- [ ] `polyoxide-core/src/keychain.rs`: `Field`, `StoredCredential`, `store`/`show`/`delete` and `Invalid`, with unit tests (validation and unknown fields) that need no keyring.
- [ ] polyoxide-polymarket: the `StoredCredential` impls on `Account` and `BuilderAccount`, and `delete_from_keychain` through `keychain::delete`.
- [ ] `polyoxide-cli/src/commands/credentials.rs` and `polymarket/credentials.rs`: the generic command and Polymarket's kinds. Add parse tests for every kind and verb, and the `show` redaction test.
- [ ] `ci/test-body-diff.toml` and `docs/rename-manifest.md`:
  - the retirements and the path rows;
  - the hand-written `## Commands` section, with the `main.rs` test that each new command in it parses;
  - then `api_removals.py manifest --write`.
- [ ] Docs:
  - the CLI README, CLAUDE.md, the root README and the data-v2 INDEX;
  - deferred-work: G04's CLI half done, and DW:93 re-filed for S3;
  - then `gen_registry.py --write`.

**Acceptance Criteria:**
- Given the tree, when clippy (`-D warnings`), the workspace tests, the doctests and rustdoc (`-D warnings`) run, then all pass. The per-binary counts match the baseline's, apart from the listed retirements and the new tests.
- Given the tree, when cargo-hack `--each-feature` runs, then it passes, `polyoxide-cli` with and without `keychain` included.
- Given the tree, when `test_body_diff.py` and `manifest --strict` run, then they report 0 diffs and only the listed retirements, and exit 0.
- Given the scripts job, when its pytest, `venue_gate.py` and `gen_registry.py --check` run, then all pass.
- Given `polyoxide-cli/src`, when it is searched for `enum OutputFormat`, a stream loop, `ctrlc` or `select!`, then the only hits are `stream.rs` and the dataset writer's enum.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Design Notes

**The runner's contract.** A command implements a small feed trait, or passes closures. The runner owns the limits and the end of the run, and the command owns the meaning of each event:

```rust
pub enum Handled { Printed, Skipped }
// event: write updates to `out` and markers to `err`; a BrokenPipe on `out` ends the run quietly.
// error: write a skippable error to `err` and return Ok, or return the error to end the run.
```

Sports' filter, Binance's per-row lines and the backfill marker of rtds stay in their commands.

**Why retire and not drift.** A test that names `["polyoxide", "gamma"]`, or matches `Commands::Gamma`, is testing the old command path. `[literals]` cannot turn one literal into two, and S2 allows no DRIFT row but R6. So each such test retires with a reason row, and a test of the new path replaces it. It keeps its old name where the name still reads true.

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass
- `CARGO_INCREMENTAL=0 cargo test -p polyoxide-cli --no-default-features` -- expected: all pass without `keychain`
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run it alone)
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, the listed retirements, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `python3 scripts/venue_gate.py && python3 scripts/gen_registry.py --check` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass

**Manual checks:**
- `polyoxide --help`, `polyoxide polymarket --help` and `polyoxide binance usdm stream --help` show the new tree.
- `polyoxide ws` is refused.
