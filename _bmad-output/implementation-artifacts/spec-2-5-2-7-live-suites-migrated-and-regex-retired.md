---
title: 'Stories 2.5, 2.6 and 2.7: Migrate every live suite to the toolkit, then retire the regex classifier'
type: 'refactor'
created: '2026-10-08'
status: 'ready-for-dev'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-2-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-2-3-2-4-test-toolkit-and-tag-classifier.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-2.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Live-test failures are still classified by guessing from panic text. 416 unwraps, and about 35 bare `panic!` or `assert!` sites, print no tag.

**Approach:**
- **Story 2.5:** migrate the HTTP live suites to `polyoxide-test-support`, using `or_fail`, `fail`, the loaders, `environmental` and `transient`.
- **Story 2.6:** migrate the socket suites the same way.
- **Story 2.7:** once `live_unwraps.py`'s baseline is empty, delete the regex fallback and convert its tests to tag-table tests, one per original case.

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **One bundle, one review layer.** Work file by file.
- **Unwraps not on a polyoxide `Result`:**
  - **Absence the outside world causes** (no suitable market, an empty book) → `unwrap_or_else(|| environmental("…"))`.
  - **An assertion on response data, or a parse, serde, `SystemTime`, tempdir, clap or `JoinError` in test code** → keep the `.expect`, with a `// live-unwraps: <reason>` opt-out. It stays untagged, so `real`.
  - **A bare stream ending** (`next()` returns `None`) → `transient("the server ended the connection")`. On a supervised tier, where the stream ends only after a fatal error, it stays `real` and is opted out, except Binance `live_ws` :111, which is `transient` per Story 2.6.
  - **A timeout:** `environmental` where a quiet feed is documented as legitimate (sports, rtds, perps "legitimately time out"), and `real` (opted out) where silence is the property under test (perps `live_ws` :90 and :143).
  - **A foreign error** (reqwest, tungstenite) is wrapped in the venue's own error (`ApiError::Network`, `SportsError::Transport`, `UsdmWsError::from`, `WebSocketError::Connection`), then goes through `or_fail` or `fail`.
- **Outside-world guards become precondition checks.** `assert!(found_market, …)` turns into `if !cond { environmental(…) }`. This changes the guard's form but not what the test asserts.
- **Data-dependent silent returns and `eprintln!` skips become `environmental`.** These are in gamma (:955, :997, :1143, :1149, :1178, :1191-1205, :1222, :1240) and clob (:534, :720, :856). A test that asserted nothing must not report ok. Informational `eprintln!` lines that continue the test stay.
- **Credentials go through the loaders** (`load_env(&[..]).or_auth_gated()`, plus `keychain` under `cfg(feature = "keychain")`). That includes relay's three soft-skips and clob's builder-code soft-skip, which become auth-gated. Locally they now fail rather than pass, matching clob's own doc comment that calls the soft-skip a failure mode.
- **Swallowed probe errors.** `if let Ok(book)` probes keep their last error and `fail` with it when every probe errored, so an outage is not reported as "no suitable market".
- **Sockets:**
  - sports :148 and :215 ("ended without a close frame") become `transient`, as AD-14 defines it;
  - sports :186 `Disconnected`: `Stale` stays an untagged `real` panic, because staleness is the property under test, and every other reason goes through `fail`;
  - Binance `live_ws` :234 (pong unanswered) keeps an opted-out `.expect`, so it stays `real`, because it is the property under test.
- **The counter also counts bare failure sites.** `live_unwraps.py` counts `panic!(`, `unreachable!(` and `assert!`-family macros whose message names an error or status, in live targets and their `mod` files, with the same opt-out. Then an empty baseline really means every failure site is tagged or deliberately opted out.
- **After 2.7, live targets read the environment only through loaders.** A check forbids `std::env::var` in them. `gen_registry.py`'s `env_names()` then reads only loader calls, and its `ENV_LITERAL` scan and `LOADERS` table are retired.
- **Regex-era tests become tag-table tests.** Each case becomes a row: the tag line plus the original text, with the same verdict. Where AD-14 (with amendment A2-1) disagrees, the new verdict stands and the row is marked: close 1000 → `transient`, and NoAnswer `Display` → `transient`. Binance 418 and WAF 403 stay `real` under A2-1, so `test_other_binance_refusals_are_not_environmental` keeps its verdict. Each row that names a constructible error gets a Rust twin asserting its tag.

## Boundaries & Constraints

**Always:**
- **NFR7:** test names, assertions and per-suite counts stay unchanged: 154 tests (gamma 46, clob `live_api` 45, data 26, clob `live_session_keys` 1, binance `live_api` 4, perps `live_api` 4, perps `live_ws` 2, cli 5, binance `live_ws` 4, relay 7, clob `live_ws` 2, sports 5, rtds 3). Report the counts before and after.
- **Disk:** build with `CARGO_INCREMENTAL=0` and `-j 4`, compile-check per crate (`cargo test -p X --tests --no-run`), and do not run `api_removals.py`.
- **Each live crate gains** `polyoxide-test-support` as a path-only dev-dependency.
- **Env-name arguments to loaders are inline literals,** matching each target's declared `secrets`.
- **D1's API, as committed in 0de9baa** (read `polyoxide-test-support/src/` and its spec): `ResultExt::or_fail(ctx)`, `fail(ctx, &err)`, `environmental(reason)`, `transient(reason)`, `load_env(&[..])`, `optional_env(name)`, `keychain(service, &[(env, key)])`, `Missing::or_auth_gated()` and `Creds::get`.
  - **The tag map:** `Restricted` is `environmental` only when it is not a fault (amendment A2-1), so Binance's 418 and WAF 403 are `real`.
  - **The classifier's rule:** a tag counts only when it directly precedes the final `panicked at`, and a `real` tag anywhere wins.
- **`live_unwraps.py`'s baseline** also counts `opted_out` per file, and refuses a rise through `--lower`. The opt-outs this migration adds are legitimate. Raise the `opted_out` counts by hand in `scripts/live_unwraps.baseline.json`, each opt-out line carrying its reason, so review sees the increase. Lower `unwraps` with `--lower`.

**Never:**
- Change library code.
- Delete or weaken an assertion.
- Run the live suites against production with credentials. Running the credential-free suites (gamma, data, rtds, sports, perps, binance) once each, to see real tag lines, is allowed.

</frozen-after-approval>

## Code Map

- **Per-file counts** (tests / unwraps / on a polyoxide `Result`):
  - gamma 46/116/80, clob `live_api` 45/98/81, data 26/61/56, clob `live_session_keys` 1/26/22;
  - binance `live_api` 4/23/16, perps `live_api` 4/23/21, perps `live_ws` 2/18/12;
  - cli 5/16/1, plus 3 eyre chains;
  - binance `live_ws` 4/12/7, relay 7/10/9, clob `live_ws` 2/5/5, sports 5/4/1, rtds 3/3/3, binance `common` 0/1/0.
- **Environmental sites:**
  - "no suitable market": clob `live_api` :157, :1067, :1169; `live_session_keys` :239; data :250; perps `live_api` :50 and :131; perps `live_ws` :39; binance `live_api` :48 and :54;
  - "legitimately time out": sports :91 and :252; rtds :41, :149 and :210; perps `live_ws` :68; cli :181;
  - 451: binance through `BinanceError`/`UsdmWsError`; cli :225; binance `live_api` `raw()` :188.
- **Socket sites:**
  - sports: :92-93, :120-124, :148, :186, :194, :215, `connect_failed` :50-57, `connect_raw` :64-69;
  - rtds: :36, the `stream error` arms :33, :54, :77, :121 and :205;
  - binance `live_ws`: :111, :146, :149, :154, :156, :234;
  - perps `live_ws`: :55, :112-116 (a spawned task), :159;
  - clob `live_ws`: :97-131 probe, :216.
- **Credential helpers:**
  - clob `load_account` :38-48 and `BUILDER_CODE` :780;
  - clob `live_ws` `l1_account` :40-61;
  - `live_session_keys` `load_fixture` :60-97 (`BUILDER_PASS_PHRASE` goes through `optional_env`);
  - relay `client_with_builder_env` :24-40 and `client_with_relayer_api_key_env` :43-56.
- **Hidden untagged sites** (about 35): gamma :1225; every rtds `stream error` arm; sports :119 and :193; binance `live_ws` :97, :112, :222, :233; cli :64, :69 and :225.
- **Regex-era tables** in `.github/scripts/tests/test_classify_failures.py`: RETRIABLE_ARMS (19), NON_RETRIABLE (12), WEBSOCKET_DROPS (15), WS_FAULTS (12), BINANCE_REGION_BLOCKS (5), `test_other_binance_refusals_are_not_environmental`, the plain-text tests, and the fixture and CLI tests, whose fixtures need tag lines.
- **Docs to rewrite in the same change:**
  - CLAUDE.md: the classification bullets after `<!-- generated:end claude-nightly -->`, the "remove the auth patterns from `AUTH_GATED_RE`" paragraph, and D1's fallback paragraph;
  - SELF-HEALING.md: its verdict table, its AUTH_GATED_RE mentions, "encode its panic message in the classifier", and "Tuning classification", all outside generated markers;
  - test doc comments citing the regex: clob `live_api` :31-37 and :111, `live_ws` :35-39, `live_session_keys` :25-26 and :55-59, sports :7-13 and :48, binance `live_api` :186, binance `live_ws` :12-15, cli :169.

## Tasks & Acceptance

**Execution:**
- [ ] Each live crate's `Cargo.toml` -- add the dev-dependency `polyoxide-test-support = { path = "../polyoxide-test-support" }`.
- [ ] **Story 2.5, the HTTP suites:** gamma, data, clob `live_api`, clob `live_session_keys`, relay, perps `live_api`, binance `live_api` and cli. Migrate each per the Decisions, then lower its baseline counts.
- [ ] **Story 2.6, the socket suites:** clob `live_ws`, perps `live_ws`, binance `live_ws` and its `common`, rtds and sports. Migrate each per the Decisions. The baseline becomes empty.
- [ ] `scripts/live_unwraps.py` and its baseline -- also count the bare failure sites from the Decisions, and end with every count at 0.
- [ ] A check, in `test_live_registry.py` or `live_unwraps.py`, that no live target calls `std::env::var`. Then `gen_registry.py` `env_names()` reads only loader calls, and `ENV_LITERAL` and `LOADERS` are retired, with their tests updated.
- [ ] **Story 2.7:** `.github/scripts/classify_failures.py` deletes `AUTH_GATED_RE`, `ENVIRONMENTAL_RE` and `TRANSIENT_RES`, so an untagged log is `real`. `.github/scripts/tests/test_classify_failures.py` turns each original case into a tag-table row with the same verdict, or a marked new one, and the fixtures gain tag lines.
- [ ] Rust twins: a test in `polyoxide-test-support`, or per crate, asserting the tag for each constructible error named in the tables.
- [ ] Docs: rewrite CLAUDE.md, SELF-HEALING.md and the test doc comments, as listed in the Code Map.

**Acceptance Criteria:**
- Given every live target, when `live_unwraps.py` runs, then every count is 0, and so is the bare-failure count.
- Given the classifier, when a log has no tag, then it is `real`, and no regex remains.
- Given each original regex-era case, when its tag-table row runs, then the verdict is the same, except the marked AD-14/A2-1 rows.
- Given each live suite, when its tests are listed (`cargo test -p X --test live_* -- --list --ignored`), then the names and counts equal the baseline.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `cargo test -p <each live crate> --tests --no-run -j 4` with `CARGO_INCREMENTAL=0` -- expected: builds.
- `cargo clippy --workspace --all-targets --all-features -j 4 -- -D warnings` -- expected: clean.
- For each crate, `cargo test -p <crate> --test <target> -- --list --ignored | wc -l` -- expected: the counts above.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass.
- `python3 scripts/live_unwraps.py && python3 scripts/gen_registry.py --check` -- expected: exit 0.
- Credential-free live suites run once each (gamma, data, rtds, sports, perps, binance), with tag lines seen for any failure -- expected: recorded in Implementation Notes.
