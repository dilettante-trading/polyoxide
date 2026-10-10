---
title: 'Stories 1.6, 1.7 and 1.8: Workspace hygiene gates, the S1 removal gate, and the agent guide'
type: 'feature'
created: '2026-10-08'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'a35a8ee3b2097b98dc70ac6f6f4f9091527e3105'
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
- [x] Root `Cargo.toml` -- `resolver = "3"`. `Cargo.lock` is unchanged.
- [x] `.github/workflows/ci.yml`:
  - **`msrv` job:** `dtolnay/rust-toolchain@1.91`, sccache, then `cargo check --workspace --all-features` and `cargo doc --no-deps --workspace --all-features`, without `-D warnings`.
  - **`features` job:** `taiki-e/install-action` with `cargo-hack@0.6.45`, sccache, then `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`.
  - **`removals` job:** `env: S1_BASELINE: v0.38.1`, `fetch-depth: 0`, `dtolnay/rust-toolchain@1.99.0`, `taiki-e/install-action` with `cargo-semver-checks@0.51.0`, then `python3 scripts/api_removals.py check --baseline "$S1_BASELINE"`. A comment says the tool and the toolchain upgrade together.
- [x] `scripts/api_removals.py` -- new. `check --baseline REV [--release-type patch]`:
  - runs `cargo semver-checks --workspace --baseline-rev REV --release-type patch --color never`, plus `--exclude` for each workspace member absent from REV's tree (worked out with `git ls-tree`);
  - parses the output into removal keys;
  - exits 1 listing the keys not in `docs/s1-removals.md`;
  - exits 2 if cargo-semver-checks exits 101;
  - then runs the doc-hidden compile test: a scratch crate under `target/api-removals/` that path-depends on the workspace crates with the needed features and `use`s every listed doc-hidden path not marked removed, then `cargo check`s it.

  `release` mode (for `release.yml`) runs without `--release-type`, against `--baseline v<prev>`, and fails on any lint failure.
- [x] `docs/s1-removals.md` -- new.
  - Front matter: `baseline: v0.38.1`.
  - A "Removed" section, empty and cumulative until the S1 release.
  - A "Doc-hidden paths consumers import" section listing the paths above with their features.
  - A short header explaining how a PR adds a line.
- [x] `.github/workflows/release.yml` -- a `semver` job after `version`, gated on `should_release`. It checks out with tags, uses the pinned toolchain and tool, and runs `api_removals.py release --baseline <previous tag>`, where the previous tag is the newest `v*` tag reachable from the commit's parent whose version sorts below the workspace version (amended in review; see the Spec Change Log). Both `publish` and `publish-python` `need` it.
- [x] `.github/scripts/tests/test_api_removals.py` -- new. Cover:
  - parsing captured outputs: removal and non-removal lints, `inherent_method_missing`, file-path stripping, multiple crates;
  - exit 100 versus 101;
  - listed versus unlisted removals;
  - the exclude list for a crate absent from the baseline;
  - the doc-hidden section parse.
- [x] Proof against a scratch removal:
  1. Temporarily delete one public item, for example a `pub fn` in `polyoxide-sports`.
  2. Run `api_removals.py check`; it must fail naming it.
  3. Add it to `docs/s1-removals.md`; it must pass.
  4. Restore both byte-for-byte.
  5. Repeat with one doc-hidden path, which the compile test must catch.

  Record the commands and outputs in Implementation Notes.
- [x] `docs/ARCHITECTURE.md` -- new. A header saying it is copied verbatim from the spine's `ARCHITECTURE-GUIDE.md`, is regenerated only from the spine, and is not edited by hand. Then a `gen_registry.py` region `architecture-stage` stating "The workspace is in stage S1". Then the guide.
  - `gen_registry.py` gains that renderer, plus `[workspace.metadata.polyoxide] stage = "S1"`.
  - A test pins the body to the guide byte for byte.
- [x] `CLAUDE.md` -- a new `## Architecture guide` section linking `docs/ARCHITECTURE.md` and `docs/MUTANTS.md`, saying the guide is regenerated from the spine. The CI paragraph names the three new jobs.
- [x] `docs/MUTANTS.md` -- new. One row per rule (a) to (e): file:line, the mutation, and the tests that must fail. Rule (a) carries a "not yet covered" list of its other sites, pointing to Story 3.1. Apply each mutation, run the named tests (`cargo test -p … -j 4 <name>`), confirm they fail, restore, and record the results in Implementation Notes.

**Acceptance Criteria:**
- Given today's tree, when the three new jobs' commands run, then `features` and `removals` pass locally, and `msrv` passes locally if 1.91 was obtainable, else it is marked unverified.
- Given a removal not in `docs/s1-removals.md`, or a removed doc-hidden path, when `api_removals.py check` runs, then it fails naming it.
- Given a patch-version release whose diff removes an item, when `release.yml` runs, then the `semver` job fails before anything publishes.
- Given clob built without reqwest's help, when `rustls` resolves, then it has `std`.

## Implementation Notes

All local runs used `-j 4` (or `CARGO_BUILD_JOBS=4`), `RUSTFLAGS` unset, and target dirs under the worktree's `target/`. Nothing was committed or pushed.

**Toolchains and tools.** Official Rust 1.91.0 and 1.99.0 (`rustc`, `rust-std`, `cargo`) were installed from static.rust-lang.org into `target/rust-1.91.0/prefix` and `target/rust-1.99.0/prefix`, each tarball sha256-checked against its channel manifest (nix-ld runs them). `cargo install cargo-hack@0.6.45 --locked` and `cargo install cargo-semver-checks@0.51.0 --locked` went into `~/.cargo/bin`.

**Story 1.6.**
- `resolver = "3"`: `cargo metadata --locked` and `cargo check --workspace --all-features -j 4` leave `Cargo.lock` byte-identical (`git diff --exit-code Cargo.lock`).
- `msrv` job, verified on 1.91.0: `cargo check --workspace --all-features` finished in 57 s and `cargo doc --no-deps --workspace --all-features` passed, with no `warning:` or `error` lines.
- `features` job, verified on 1.99.0: `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private -j 4` ran 54 checks, exit 0, in 2 min 57 s. The only warning was the future-incompat note for `proc-macro-error2`. Every manifest was restored afterwards.
- The `removals` job also installs sccache, because `ci.yml` sets `RUSTC_WRAPPER: sccache` for every job, and it has `timeout-minutes: 60`. A cold run took 8 min 22 s locally.

**Story 1.7.**
- `scripts/api_removals.py` follows the spec, plus these guards. Each one stops a removal getting through silently.
  - Any exit other than 0 or 100 is exit 2.
  - An exit of 100 with no failure read, or 0 with one, is exit 2.
  - A crate the tool never named in a `Checking` line is exit 2.
  - A lint block with no readable item is exit 2.
  - A missing `## Removed` or `## Doc-hidden paths consumers import` heading is exit 2.
  - A front-matter `baseline` that differs from `--baseline` is exit 2.
  - A missing baseline rev is exit 2, and the message points at `fetch-depth: 0`.
- Keys deduplicate, because 0.51.0 prints one line per importable path and some templates name only the type (`MatchUpdate::key` twice).
- The tool's stderr shares stdout's pipe. It names each crate on stderr and reports that crate's lints on stdout, and only one pipe keeps the two in order.
- The scratch crate copies the workspace `Cargo.lock`, has its own `[workspace]` table, and builds in `target/api-removals/target`.
- `release.yml` needed the parent commit's version, so `scripts/publish_order.py` gained a `parent-version SHA` subcommand, which wraps the existing `parent_version()`. It exits 1 on a root commit. Two tests in `test_publish_order.py` cover it.
- Workflow tests were added.
  - `test_ci_workflow.py`:
    - the MSRV job's toolchain equals `rust-version`;
    - neither `-D warnings` nor `RUSTDOCFLAGS` appears in the msrv job;
    - the cargo-hack pin and command;
    - the removals job's env, `fetch-depth` and command;
    - the cargo-semver-checks and toolchain pins are the same in `ci.yml` and `release.yml`;
    - no new job has an `if`, `needs` or secret.
  - `test_release_workflow.py`: `semver` is gated on `should_release`, both publish jobs `need` it, it checks out the SHA with `fetch-depth: 0`, and it only reads.
- Fixtures: `.github/scripts/tests/fixtures/semver-checks/{clean,mixed}.txt` are real 0.51.0/1.99.0 output. The worktree path in them is replaced with `/home/runner/work/polyoxide/polyoxide`, and `PROVENANCE.md` lists the four scratch edits behind `mixed.txt`. Those edits were reverted, and `git diff --exit-code` was clean afterwards.
- Proof against a scratch removal, with backups in the scratchpad and sha256 checked on restore:
  1. **Run A.** Deleted `SportsWsBuilder::connect_timeout` (`polyoxide-sports/src/supervised.rs:165-170`) and the doc-hidden `polyoxide_perps::ws::frame_from_text_for_tests` (`polyoxide-perps/src/ws/mod.rs:48-59`). `python3 scripts/api_removals.py check --baseline v0.38.1` exited 1 and printed:
     - `::error::Public items removed since v0.38.1 that docs/s1-removals.md does not list. … polyoxide-sports inherent_method_missing: SportsWsBuilder::connect_timeout`;
     - then `error[E0432]: unresolved import polyoxide_perps::ws::frame_from_text_for_tests --> src/lib.rs:8:5`;
     - then `::error::Paths listed in docs/s1-removals.md that no longer compile: polyoxide_perps::ws::frame_from_text_for_tests.`
     cargo-semver-checks itself reported nothing for the doc-hidden deletion.
  2. **Run B.** Added `` `polyoxide-sports inherent_method_missing: SportsWsBuilder::connect_timeout` `` under `## Removed` and appended `**Removed**` to the perps entry. The same command exited 0: `1 removals (1 listed)`, and 13 paths compiled.
  3. **Restore.** Restored all three files byte for byte (`sha256sum -c` OK). On the restored tree the command exits 0: `0 removals (0 listed), and 0 other changes`, and 14 paths compiled.

**Story 1.8.**
- `docs/ARCHITECTURE.md` is a 6-line "Copied from the spine's guide; do not edit." header, then the `architecture-stage` region, then the guide byte for byte.
  - `test_the_architecture_guide_is_the_spines_guide_byte_for_byte` fails when one heading's case is changed, and passes once it is restored.
  - `gen_registry.py` validates `[workspace.metadata.polyoxide] stage` against `^S[1-9][0-9]*$`, and rejects unknown keys in that table (`stage`, `mirrors`).
- Mutants. Each was applied, its tests were run (`cargo test -p <crate> <target> -j 4 -- <names>`), and the file was restored; `git diff --exit-code` was clean afterwards. All named tests passed unmutated first.

  | Mutant | Failed | Passed |
  | --- | --- | --- |
  | (a) core: `note_rate_limited` moved into the retry branch | data `a_429_makes_the_next_request_wait…` (1532) | data 1509 |
  | (a) binance: the hold only when a retry is left | binance 626, 662 | — |
  | (b) `unwrap_or(computed)` | core 509 | core 532, data 1509, data 1532 |
  | (b) zero honoured: `>= 0.0` plus `unwrap_or(computed)` | core 509, 532; data 1509, 1532 | — |
  | (c) unconditional assignment | core 1907 | core 1925 |
  | (c) sleep once | core 1925 | core 1907 |
  | (d) `.allow_burst(count)` | core 244, 261, 284 | — |
  | (e) `&&` → `\|\|` at :92 | clob error.rs 348 | error.rs 273, 285, 293, 306, 317; mock_api 3365, 3399, 3462 |
  | (e) no lowercasing | error.rs 273, 285, 293, 306; mock_api 3365, 3399 | error.rs 317, 348; mock_api 3462 |
  | (e) a 5xx body classified too | mock_api 3462 | error.rs (all six) |

**Review patches (2026-10-08).** Each finding in the Review Triage Log's "Patched" list was fixed with the smallest change that does the job. Only the tests covering the edited files were run: `test_api_removals`, `test_publish_order`, `test_release_workflow`, `test_ci_workflow`, `test_gen_registry` and `test_mutants_ledger`, 385 passed.
- **Release baseline.**
  - `publish_order.py previous-tag SHA` prints the newest `v*` tag in `git tag --list 'v*' --merged SHA^` whose version (`_semver_key`) sorts below the workspace version. It exits 1 when there is none.
  - `release.yml` runs `BASELINE=$(publish_order.py previous-tag "$SHA")`, then `api_removals.py release --baseline "$BASELINE"`.
  - `parent-version` and its tests are gone; `parent_version()` stays for `decide`.
  - Tests cover an empty commit after an untagged bump, a re-bump over an untagged version, tags at or above the version, tags that are not versions, no tag at all, and a failed lookup.
- **Keys.**
  - The format is now `<crate> <lint>: <item> (<file>)`. The file is crate-relative, with neither the extraction directory nor a line number.
  - `LOCATION` is anchored to the end of the line, with the path a single word, so an item named `at` or `in` is never cut.
  - The new real capture `twins.txt` shows polyoxide-data's two `ListTrades::limit` getting distinct keys, `(src/api/trades.rs)` and `(src/v2/api/feeds.rs)`.
  - The keys in the earlier proof runs above used the old format.
- **Baseline members.**
  - Presence is decided by package name, read with `git show <rev>:Cargo.toml` and `git show <rev>:<member>/Cargo.toml`, with `publish.workspace` inherited. A glob member is exit 2.
  - Each `--exclude` prints a `::warning::`.
  - Each publishable baseline package missing now is a `<crate> crate_missing` removal. In `release` mode it fails unless the bump raises the 0.x minor (`raises_minor`).
- **Compile test.**
  - `docs/s1-removals.md` lists items, with `a::b::{C, D}` parsed: the fixtures constants and `test_server::{Script, ScriptedServer}` the CLI's and the crates' own tests import, plus the perps functions and the type aliases.
  - One scratch crate per (crate, sorted features) under `target/api-removals/<crate>[+features]/`, each checked on its own, all sharing `target/api-removals/target`.
  - `cargo check --message-format json --color never`. Error lines are taken only from `error` diagnostics whose `manifest_path` is the scratch crate's and whose primary span is its own `src/lib.rs`, and the log shows rustc's rendered text.
  - Real captures: `compile-dependency-error.txt` has a dependency error at its own `src/lib.rs:5`, which the old regex would have blamed on the first import; `compile-unresolved-import.txt` has a scratch-side error at line 13.
- **Release mode.**
  - It now runs the compile test too.
  - AC 3 was proved with the real tool. With backups and sha256, the workspace was bumped to `0.38.2` in `Cargo.toml` and `SportsWsBuilder::connect_timeout` was deleted (`polyoxide-sports/src/supervised.rs:165-170`). Then `env -u RUSTFLAGS PATH=target/rust-1.99.0/prefix/bin:~/.cargo/bin:$PATH CARGO_BUILD_JOBS=4 python3 scripts/api_removals.py release --baseline v0.38.1` printed `Checking polyoxide-sports v0.38.1 -> v0.38.2 (minor change)` for each of the 11 crates, then `--- failure inherent_method_missing`, then `::error::Changes since v0.38.1 that a bump to 0.38.2 does not allow. A release that removes or breaks a public item must raise the 0.x minor (AD-25):` and `  polyoxide-sports inherent_method_missing: SportsWsBuilder::connect_timeout (src/supervised.rs)`.
  - The compile test then built all eight groups (binance `test-server`; perps `test-server`; perps `ws`; rtds `test-fixtures`; sports `test-server`; clob, data and relay with default features). The run exited 1 after 4 min 38 s.
  - `Cargo.toml`, `Cargo.lock` and `supervised.rs` were restored, and `sha256sum -c` was OK.
- **Exit codes and the listing.**
  - `main` turns `OSError`, `ValueError` (which covers `JSONDecodeError` and `TOMLDecodeError`) and `KeyError` into exit 2 with `::error::cannot decide`.
  - A `## Removed` entry with nothing after its key is refused.
- **gen_registry.**
  - `STAGE.fullmatch`.
  - `STAGE_ANCHOR` is tested against the guide's headings, slugged the way GitHub does it.
  - `docs/ARCHITECTURE.md` has an `architecture-guide` region that `--write` fills from `GUIDE` byte for byte; a guide without a final newline is refused.
  - The byte-pin test now reads the region. A test blanks the region and checks that `--write` restores the file exactly.
- **Pins and ledger.**
  - `PROVENANCE.md` records `Tool: cargo-semver-checks 0.51.0`, `Toolchain: Rust 1.99.0`, the commit and every capture command, and a test checks it against both workflows' pins.
  - `test_mutants_ledger.py` checks every `file:line` citation in `MUTANTS.md`, both ends of a range included, against a recorded snippet. Every citation is now a full path.
- **Cache.**
  - `actions/cache@v4` caches `target/semver-checks/git-*`, keyed `semver-checks-${{ env.S1_BASELINE }}-rust-1.99.0-cargo-semver-checks-0.51.0`, before the run.
  - Only the baseline's tree is cached. Locally it is 5.6 GB of the 12 GB; the current crates' builds change with every PR.
  - A test ties the key to both pins.
- **CLAUDE.md.**
  - CLAUDE.md describes the code as it is, and the guide describes the target. A rule here stands until the commit that supersedes it lands.
  - The `semver` sentence now says it fails on any major-level change on a 0.x patch bump, that it runs the compile test, and that the remedy is a new 0.x-minor bump commit on `main`.
  - The red-build warning now covers every CI job.

## Spec Change Log

- **Review amendment (2026-10-08), not a loopback.**
  - **Trigger:** all three layers found that the `semver` job's baseline, `v<parent version>` (this spec's Tasks line and Code Map), fails exactly where `decide` row 2 releases. That covers a fix-forward or empty commit after a red bump, which is CLAUDE.md's documented recovery, and a re-bump after a failed gate. The parent's version is untagged there, so exit 2 withholds the release.
  - **Amended:** the Tasks line now names the newest `v*` tag reachable from the parent whose version sorts below the workspace version.
  - **Why no loopback:** the code is otherwise verified, including 12 GB of real cargo-semver-checks runs. The fix is a small subcommand plus tests. Reverting and re-deriving the bundle would cost far more than it protects, so the orchestrator patched in place.
  - **Known-bad state avoided:** a release withheld on its documented recovery path.
  - **KEEP:** everything else in the bundle.

These are where the implementation goes beyond the Code Map. None changes the frozen intent.
- **Removal lint ids.** They also include `trait_removed_associated_constant` and `trait_removed_associated_type`, which remove public items. 0.51.0 has exactly 21 `*_missing` lints, as the Code Map says.
- **Type aliases.** The four public type-alias paths (`polyoxide_clob::DynSigner`, `polyoxide_clob::account::DynSigner`, `polyoxide_data::v2::PageStream`, `polyoxide_relay::DynSigner`) are in the compile test's list too, under the doc-hidden section, because the Code Map notes that 0.51.0 does not check aliases either.
- **MUTANTS.md.**
  - The Code Map's rule (b) row names client.rs:532 and data :1509, but neither fails under `unwrap_or(computed)`. The `> 0.0` filter already drops a zero `Retry-After`, so a second (b) row, "zero honoured", was added, and it kills both.
  - mock_api :3462 fails under neither (e) mutation. A third (e) row, "a 5xx body classified too", kills it.
  - error.rs:317 fails under none, which MUTANTS.md says outright.
  - Binance's own call site has a covered row of its own; the decision had cited only core's `Request` loop.
- **`publish_order.py previous-tag`.** This subcommand was added for `release.yml`. It replaced a `parent-version` subcommand in review, which nothing else used.

## Review Triage Log

All three layers ran: blind (15 findings), edge-case (10) and verification-gap (0 gaps, 7 other findings).

**Patched:**
- **High:** the release baseline `v<parent version>` blocks untagged-recovery releases (blind, edge, gap). This is a spec error; see the Spec Change Log.
- **Medium:** removal keys collide across same-named types, and the `LOCATION` regex is unanchored (blind, gap). polyoxide-data has about 25 v1/v2 twin names.
- **Medium:** a moved crate is silently excluded and a deleted crate is never reported (blind, edge, gap).
- **Medium:** the compile test imports modules, not items, and unifies features across entries (blind, edge).
- **Medium:** release mode skips the compile test, and AC 3 has no real-tool proof (blind, edge).
- **Low:**
  - `ERROR_LINE` blames the wrong path and output is not colour-stripped (blind, edge, gap). The exit code was still 1;
  - uncaught exceptions exit 1 instead of 2 (edge);
  - an empty `## Removed` entry is accepted (blind);
  - `STAGE` accepts a trailing newline (edge);
  - the guide anchor is untested and the guide splice is done by hand (blind, edge);
  - fixtures are not tied to the pins (blind);
  - `MUTANTS.md` citations have no staleness check (blind);
  - no cache for the baseline build (blind);
  - CLAUDE.md's "the guide wins", its understated `semver` sentence with no remedy, and the red-CI warning not extended (blind).

**Deferred:**
- extra `MUTANTS.md` rules beyond Story 1.8's five (blind). That is out of the AC's scope;
- unverified (would be medium): cross-crate re-exports may be invisible to cargo-semver-checks 0.51.0 (gap). Both are in deferred-work.md.

**Rejected:**
- the sprint-status mismatch (blind): orchestrator bookkeeping, synced at completion;
- a prose code span read as a feature (edge): low, and it fails loudly with a compile error, never silently.

## Verification

**Commands:**
- `cd .github/scripts && uv run pytest tests/ -v` -- expected: all pass.
- `python3 scripts/gen_registry.py --check` -- expected: exit 0.
- `python3 scripts/api_removals.py check --baseline v0.38.1` -- expected: exit 0.
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private -j 4` -- expected: success.
- `cargo check --workspace --all-features -j 4 && git diff --exit-code Cargo.lock` -- expected: success, with the lockfile unchanged.
