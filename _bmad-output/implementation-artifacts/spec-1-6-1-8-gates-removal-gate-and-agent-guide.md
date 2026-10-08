---
title: 'Stories 1.6, 1.7 and 1.8: Workspace hygiene gates, the S1 removal gate, and the agent guide'
type: 'feature'
created: '2026-10-08'
status: 'ready-for-dev'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- Nothing checks the MSRV.
- Nothing checks that each feature compiles on its own, rather than only through unification with others.
- Clob's `rustls` lacks `std` and compiles only because reqwest and alloy enable it (DRIFT R5).
- Nothing stops a PR from removing a public item that prader-rs imports.
- The architecture guide and the mutation-tested rules live only in planning files.

**Approach:**
- **Story 1.6:** set `resolver = "3"` and declare `rustls` once at workspace level, with R5 in its own commit. Add MSRV and cargo-hack jobs.
- **Story 1.7:** add a removal-gate job. cargo-semver-checks runs against the S1 start tag, and `scripts/api_removals.py` fails on any removal missing from `docs/s1-removals.md`. A compile test covers the doc-hidden paths that consumers import. `release.yml` refuses removals on a patch bump.
- **Story 1.8:** `docs/ARCHITECTURE.md` copies the spine's guide and states the stage from metadata, CLAUDE.md links to it, and `docs/MUTANTS.md` lists the mutation-tested rules, each one proven.

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **One bundle.** The user asked on 2026-10-08 to fold stories coarsely. All three sprint keys move together. Review uses all three layers, because the removal gate can let a removal through silently.
- **The removal gate's toolchain.** It is pinned to Rust **1.99.0** together with cargo-semver-checks **0.51.0**. 0.51.0 reads rustdoc JSON v57, v60 and v61, which only 1.93–1.99 emit; 1.91 emits v56. Upgrade the two together (AD-22).
- **The baseline.** `v0.38.1` (commit 12e83164), as recorded by Story 1.1, written into `ci.yml` as `S1_BASELINE`.
- **The stage line.** AD-21 allows the current stage to come from a `gen_registry.py` region fed by metadata, so `[workspace.metadata.polyoxide] stage = "S1"` feeds a region in `docs/ARCHITECTURE.md`. The rest of that file is the guide verbatim, under a "copied from the spine's guide; do not edit" header, and a test pins the body to the guide.
- **Rule (a) in MUTANTS.md** (429 feedback before the retry decision). Its row cites the one call site a test covers: core's `Request` loop, via data's `mock_api` test. The five uncovered sites are listed as gaps that Story 3.1 closes by collapsing them into one loop. No new tests are written for loops Story 3.1 deletes.

## Boundaries & Constraints

**Always:**
- **DRIFT R5 already landed as its own commit (7c14558).** Do not touch the `rustls` declarations.
- **Never run cargo-hack's `--no-dev-deps` while another process edits manifests.** It rewrites every `Cargo.toml` and then restores them.
- **Keep cargo target dirs out of `/tmp`.** Use `-j 4`.
- **Local verification is required wherever possible:**
  - `cargo install cargo-hack@0.6.45 --locked` and `cargo install cargo-semver-checks@0.51.0 --locked` into `~/.cargo/bin` are allowed;
  - a Rust 1.91 toolchain may come from nix or rustup if obtainable;
  - otherwise state plainly that the MSRV job is unverified.
- **`api_removals.py` uses only Python's standard library**, and its parser is tested on captured fixtures.
- **Treat exit 100 from cargo-semver-checks as data and exit 101 as failure.** Filter by removal lint id. Key each removal as crate + lint id + message, without file paths.

**Never:**
- Remove a public item.
- Push.
- Change the other CI jobs, except where a task names them.
- Hand-edit `docs/ARCHITECTURE.md` below its header.
- Add the S2 report-only mode or the S3 hard mode.

</frozen-after-approval>

## Code Map

- **Root `Cargo.toml`:** `resolver = "2"` (:17), edition 2021, `rust-version = "1.91"`. `tokio-tungstenite` is in `[workspace.dependencies]` (:50); `rustls` is not.
  - A scratch copy with `resolver = "3"` left `Cargo.lock` and `cargo tree --all-features` byte-identical.
- **`rustls` stanzas**, all `version = "0.23", default-features = false`:
  - clob `Cargo.toml:52-54`: `["ring"]`, optional, **no `std`**;
  - rtds `:39-42` and sports `:41`: `["ring","std"]`;
  - perps `:32` and binance `:33`: `["ring","std"]`, optional.
- **Local tools:** cargo and rustc 1.95.0 from nix; no rustup on PATH; no 1.91 anywhere locally; jq, uv and python3 present. cargo-hack, cargo-semver-checks, nextest and cargo-mutants are absent.
- **cargo-hack `--each-feature --no-dev-deps`:** about 54 check runs, with polyoxide at 13. polyoxide-py is `publish = false`, so use `--ignore-private`. No suspected unification holes; the clob `std` gap is invisible to it, which is why R5 exists.
- **cargo-semver-checks 0.51.0:**
  - `--baseline-rev` extracts the git tree, so the checkout needs tags. Each baseline crate is rebuilt with a fresh `cargo update`, so it needs the network and resolves the newest dependencies.
  - `--workspace` skips `publish = false`, and one package missing at the baseline fails the whole run, so `--exclude` it.
  - `--release-type patch` raises every lint to required.
  - Exit codes: 0 clean, 100 lint failures, 101 error. There is no JSON output.
  - stdout prints `--- failure <lint_id>: <desc> ---`, `Failed in:`, then indented lines with `, previously in file …:N`. stderr prints `Checking <crate> vA -> vB`.
  - Removal lint ids are the 21 `*_missing`, `macro_no_longer_exported` and the `*_now_doc_hidden` family.
  - polyoxide-cli has a lib target (`pub mod commands`), so it is checked too.
- **`#[doc(hidden)] pub` items**, invisible to cargo-semver-checks:
  - binance: `usdm::ws::fixtures` and `usdm::ws::test_server`, under `test-server`;
  - perps: `ws::test_server` under `test-server`; `ws::frame_from_text_for_tests`, `ws::IncomingForTests` and `ws::incoming_from_text_for_tests` under `ws`;
  - rtds: `fixtures` and `test_server`, under `test-fixtures`;
  - sports: `fixtures` and `test_server`, under `test-server`.

  Type aliases are also unchecked by 0.51.0: `polyoxide_data::v2::envelope::PageStream` and two `DynSigner` aliases (clob `account/wallet.rs:19`, relay `account.rs:12`).
- **`release.yml`:**
  - The `version` job runs at :19-86, with `decide` at :69-86 and `fetch-depth: 2` at :55.
  - `publish` `needs` at :93, and `publish-python` `needs` at :241.
  - The previous tag is `v<parent version>`, because the bump is the last commit before the tag (AD-25). `publish_order.py` reads the parent version.
- **Guide:** `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-GUIDE.md` (229 lines). Its stage section (:41-50) never says "S1". `docs/` exists, with no ARCHITECTURE.md or MUTANTS.md.
- **Mutation rules:**

  | Rule | Where it holds | Mutation | Tests that must fail |
  |---|---|---|---|
  | (a) 429 feedback before the retry decision | core `request.rs:155/157` (the other sites: core `client.rs:215` `get_bytes`, clob `request.rs:262`, relay `client.rs:293/392/1836`, binance `usdm/request.rs:131-140`) | move the feedback call into the retry branch | data `tests/mock_api.rs:1532`; binance `mock_api.rs:626,662` for its site |
  | (b) `Retry-After` only extends a wait | core `client.rs:154` | `unwrap_or(computed)` | `client.rs:509,532`; data `mock_api.rs:1509` |
  | (c) cooldowns only extend | `rate_limit.rs:363` and `:381-397` | an unconditional assignment; sleep once | `rate_limit.rs:1907,1925` |
  | (d) `quota()` without `allow_burst` | `rate_limit.rs:165-167` | append `.allow_burst(count)` | `rate_limit.rs:244,261,284` |
  | (e) `classify_order_kill` | clob `error.rs:88-111` | `&&` → `\|\|` at :92; drop the lowercasing | `error.rs:273,293,317,348`; clob `mock_api.rs:3365,3399,3462` |

  Measured: `cargo test -p polyoxide-core --lib -j 4` takes 20 s cold. clob takes 2–4 min cold.

## Tasks & Acceptance

**Execution:**
- [x] **R5:** landed by the orchestrator as its own commit, 7c14558. The workspace `rustls` entry has `ring` and `std`; the five socket crates use `workspace = true`, with their optionality kept; CLAUDE.md's TLS paragraph is updated; `Cargo.lock` is unchanged. Do not redo it.
- [ ] Root `Cargo.toml` -- `resolver = "3"`. `Cargo.lock` is unchanged.
- [ ] `.github/workflows/ci.yml`:
  - **`msrv` job:** `dtolnay/rust-toolchain@1.91`, sccache, then `cargo check --workspace --all-features` and `cargo doc --no-deps --workspace --all-features`, without `-D warnings`.
  - **`features` job:** `taiki-e/install-action` with `cargo-hack@0.6.45`, sccache, then `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`.
  - **`removals` job:** `env: S1_BASELINE: v0.38.1`, `fetch-depth: 0`, `dtolnay/rust-toolchain@1.99.0`, `taiki-e/install-action` with `cargo-semver-checks@0.51.0`, then `python3 scripts/api_removals.py check --baseline "$S1_BASELINE"`. A comment says the tool and the toolchain upgrade together.
- [ ] `scripts/api_removals.py` -- new. `check --baseline REV [--release-type patch]`:
  - runs `cargo semver-checks --workspace --baseline-rev REV --release-type patch --color never`, plus `--exclude` for each workspace member absent from REV's tree (worked out with `git ls-tree`);
  - parses the output into removal keys;
  - exits 1 listing the keys not in `docs/s1-removals.md`;
  - exits 2 if cargo-semver-checks exits 101;
  - then runs the doc-hidden compile test: a scratch crate under `target/api-removals/` that path-depends on the workspace crates with the needed features and `use`s every listed doc-hidden path not marked removed, then `cargo check`s it.

  `release` mode (for `release.yml`) runs without `--release-type`, against `--baseline v<prev>`, and fails on any lint failure.
- [ ] `docs/s1-removals.md` -- new.
  - Front matter: `baseline: v0.38.1`.
  - A "Removed" section, empty and cumulative until the S1 release.
  - A "Doc-hidden paths consumers import" section listing the paths above with their features.
  - A short header explaining how a PR adds a line.
- [ ] `.github/workflows/release.yml` -- a `semver` job after `version`, gated on `should_release`. It checks out with tags, uses the pinned toolchain and tool, and runs `api_removals.py release --baseline v<parent version>`. Both `publish` and `publish-python` `need` it.
- [ ] `.github/scripts/tests/test_api_removals.py` -- new. Cover:
  - parsing captured outputs: removal and non-removal lints, `inherent_method_missing`, file-path stripping, multiple crates;
  - exit 100 versus 101;
  - listed versus unlisted removals;
  - the exclude list for a crate absent from the baseline;
  - the doc-hidden section parse.
- [ ] Proof against a scratch removal:
  1. Temporarily delete one public item, for example a `pub fn` in `polyoxide-sports`.
  2. Run `api_removals.py check`; it must fail naming it.
  3. Add it to `docs/s1-removals.md`; it must pass.
  4. Restore both byte-for-byte.
  5. Repeat with one doc-hidden path, which the compile test must catch.

  Record the commands and outputs in Implementation Notes.
- [ ] `docs/ARCHITECTURE.md` -- new. A header saying it is copied verbatim from the spine's `ARCHITECTURE-GUIDE.md`, is regenerated only from the spine, and is not edited by hand. Then a `gen_registry.py` region `architecture-stage` stating "The workspace is in stage S1". Then the guide.
  - `gen_registry.py` gains that renderer, plus `[workspace.metadata.polyoxide] stage = "S1"`.
  - A test pins the body to the guide byte for byte.
- [ ] `CLAUDE.md` -- a new `## Architecture guide` section linking `docs/ARCHITECTURE.md` and `docs/MUTANTS.md`, saying the guide is regenerated from the spine. The CI paragraph names the three new jobs.
- [ ] `docs/MUTANTS.md` -- new. One row per rule (a) to (e): file:line, the mutation, and the tests that must fail. Rule (a) carries a "not yet covered" list of its other sites, pointing to Story 3.1. Apply each mutation, run the named tests (`cargo test -p … -j 4 <name>`), confirm they fail, restore, and record the results in Implementation Notes.

**Acceptance Criteria:**
- Given today's tree, when the three new jobs' commands run, then `features` and `removals` pass locally, and `msrv` passes locally if 1.91 was obtainable, else it is marked unverified.
- Given a removal not in `docs/s1-removals.md`, or a removed doc-hidden path, when `api_removals.py check` runs, then it fails naming it.
- Given a patch-version release whose diff removes an item, when `release.yml` runs, then the `semver` job fails before anything publishes.
- Given clob built without reqwest's help, when `rustls` resolves, then it has `std`.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `cd .github/scripts && uv run pytest tests/ -v` -- expected: all pass.
- `python3 scripts/gen_registry.py --check` -- expected: exit 0.
- `python3 scripts/api_removals.py check --baseline v0.38.1` -- expected: exit 0.
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private -j 4` -- expected: success.
- `cargo check --workspace --all-features -j 4 && git diff --exit-code Cargo.lock` -- expected: success, with the lockfile unchanged.
