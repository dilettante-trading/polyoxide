---
title: 'Stories 2.3 and 2.4: The test toolkit crate and failure tags, and the classifier reads tags'
type: 'feature'
created: '2026-10-08'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'e352316b27503ad540d777f6673d5bb3840b822d'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-2-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The nightly classifier guesses a failure's kind from panic text using 30-odd regexes. Live tests read credentials four different ways, and a partly configured secret set fails as `real`.

**Approach:**
- **Story 2.3.** A `publish = false` crate, `polyoxide-test-support`, prints a `polyoxide-class=<tag>` line at the failure site, from the error's class:
  - `ResultExt::or_fail`;
  - credential loaders that print `auth-gated`;
  - `environmental(reason)` and `transient(reason)`.
- **Story 2.4.** `classify_failures.py` reads that tag first, the last one winning, and falls back to its regexes only for untagged logs. `scripts/live_unwraps.py` holds a baseline that can only shrink.

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **One bundle, one review layer.**
- **The class-to-tag map follows AD-14 as written:**
  - `Restricted` → `environmental`;
  - `Network`, `Unavailable` and `RateLimited` → `transient`;
  - everything else → `real`.

  An existing regex test pins Binance's 418 and WAF 403 as `real`. That conflict is settled in Story 2.7, which converts those tests; 2.4 leaves the 27 tests untouched.
- **How the hook works.** A panic hook only ever sees the payload. So each entry point stashes its tag in a thread-local, then installs (through a chained `std::sync::Once`) a hook that prints and clears the tag before calling the previous hook. It then panics with `"{ctx}: {err:?}"`, which is `expect`'s format, so untagged-era text and `Debug` output stay the same.
- **Loaders return strings.** They return a `Creds` of `Secret<String>`, and the test builds its `Account` or `BuilderAccount`, so test-support never depends on clob or relay. Each source is all-or-nothing: the full env set, the full keychain set, or `auth-gated`. Empty counts as absent, which closes the deferred partial-credentials bug for every test that uses the loaders. Keychain lookups run only where the test calls them; tests keep their own `cfg(feature = "keychain")`.
- **Dependencies.** `dotenvy` is allowed, because "depends only on core and venue" reads as in-workspace. test-support depends on `polyoxide-core` with `keychain` and on `polyoxide-venue`.
- **`live_unwraps.py` counts every `.unwrap()`/`.expect(`** in each live target and its `mod` files. Text cannot tell a polyoxide `Result` from any other, and over-counting is the safe direction. A line may opt out with a trailing `// live-unwraps: <reason>`. The baseline must equal the actual counts and may only fall, and the classifier's regex pattern set is frozen in it.
- **The secrets check also reads loader calls.** `gen_registry.py`'s `env_names()` additionally collects the string-literal arguments of `load_env(`, `optional_env(` and `keychain(`, and refuses a non-literal argument. The existing literal scan stays until Story 2.6.

## Boundaries & Constraints

**Always:**
- The tag line is exactly `polyoxide-class=<tag>`, alone on its line, written to stderr before the panic.
- `classify()` keeps its signature, and all 27 existing tests pass unchanged.
- test-support is `publish = false` and a path-only dev-dependency everywhere: no `version` and no versioned `[workspace.dependencies]` entry. It has `[package.metadata.polyoxide] readme`, and stays on edition 2021.
- Nothing in test-support names a venue (POLYMARKET, clob, relay, binance), in code or docs, because the S2 CAP-7 gate searches it.
- A test that sets env vars does so in a child process.
- **Disk:** `/tb` was full earlier. Build with `CARGO_INCREMENTAL=0` and `-j 4`. Do not run `scripts/api_removals.py`; the orchestrator runs it.
- **The `Classify` API to build on** is in `polyoxide-venue` (Story 2.1, commit 543d1cb). Read `polyoxide-venue/src/` for the real names: `Class`, `Classify` (an `Error` supertrait), `ClassifiedError`, `Secret`.

**Never:**
- Migrate a live suite; that is Stories 2.5 and 2.6. The only exception is the one example test test-support needs.
- Delete a regex; that is Story 2.7.
- Change any error type.
- Add a regex to the classifier.

</frozen-after-approval>

## Code Map

- **AD-14** (`ARCHITECTURE-SPINE.md:354-375`) covers the tags, empty-is-absent, the chained `Once`, last-tag-wins, then the regex table, then `real`, and the shrinking `live_unwraps.py`. The review background is review-adversary P6 (`reviews/review-adversary.md:391-436`), which proposes the thread-local stash.
- **`.github/scripts/classify_failures.py`:**
  - `AUTH_GATED_RE` (:25), `ENVIRONMENTAL_RE` (:45-50), `TRANSIENT_RES` (:70-153);
  - `classify(text)` (:156-170);
  - `parse_nextest_json` (:180-203): the text is `stdout + stderr`, and nextest puts the combined capture in `stdout`, so a stderr line before the panic is kept;
  - `retry_filterset` (:206-220);
  - `classify` (:262) and `merge` (:268-298), which promotes at :279-285.
- **A latent bug:** nextest's libtest-json appends `#<attempts>` to a test's name when it ran more than once (`…$live_x#3`). `merge` looks names up bare, so a persistent transient is treated as PASS and never filed. Reproduced with a scratch merge. The test at `test_classify_failures.py:529` misses it because its retry file has no suffix.
- **Tests:** `.github/scripts/tests/test_classify_failures.py` has 27 functions and 85 cases, using six NDJSON fixtures `tests/fixtures/nextest-*.json` that carry only `stdout`.
- **Credential reads today:**
  - clob `live_api` `load_account()` (:38-48) uses dotenvy, then `Account::from_env()` (`account/mod.rs:185-208`, which accepts empty values), then the keychain under `cfg(feature = "keychain")`, then a panic;
  - `live_ws` `l1_account()` (:40-61);
  - `live_session_keys` `load_fixture()` (:60-97);
  - relay `client_with_*_env` (:24-56) returns `None`, which soft-skips;
  - clob's `POLYMARKET_BUILDER_CODE` read (:780-786).
- **Core's keychain:** `polyoxide-core` with `keychain` exports `keychain::{get,set,delete}` and `KeychainError{NotFound,Backend}` (`keychain.rs:13-75`). Its tree has keyring and reqwest, and no alloy.
- **`scripts/gen_registry.py`:** `ENV_LITERAL` (:566), `LOADERS` (:569-571), `target_source()` (:594-611), `env_names()` (:614-630).
- **`scripts/publish_order.py` `_needs`** (:186-197) refuses a versioned dev-dependency on an unpublishable member, so test-support edges must be path-only.
- **Today's unwrap/expect counts** (416 in total):

  | Target | Count |
  | --- | --- |
  | gamma | 116 |
  | clob `live_api` | 98 |
  | data | 61 |
  | clob `live_session_keys` | 26 |
  | binance `live_api` | 23 |
  | perps `live_api` | 23 |
  | perps `live_ws` | 18 |
  | cli | 16 |
  | binance `live_ws` | 12 |
  | relay | 10 |
  | clob `live_ws` | 5 |
  | sports | 4 |
  | rtds | 3 |
  | binance `common/mod.rs` | 1 |

## Tasks & Acceptance

**Execution:**
- [x] `polyoxide-test-support/` -- new crate, registered as above.
  - **Tagging:** `ResultExt::or_fail(ctx)` for `Result<T, E: Classify + Debug>`; `environmental(reason) -> !` and `transient(reason) -> !`; `fn tag_for(&Class) -> Tag`.
  - **Loaders:** `load_env(&[..]) -> Result<Creds, Missing>`, `optional_env(name) -> Option<String>` (empty counts as `None`), `keychain(service, &[(env, key)]) -> Result<Creds, Missing>` (`NotFound` or `Backend` counts as absent), and `Missing::or_auth_gated()`. Missing variables are named, never their values. `Creds::get`.
  - **The hook:** a chained `Once` with a thread-local tag. A doc comment explains why.
- [x] Root `Cargo.toml` -- add the member, then run `gen_registry.py --write`.
- [x] `polyoxide-test-support/tests/` -- prove these in child processes, by re-running `current_exe()` with `--exact <name> --ignored --nocapture` and asserting stderr:
  - every class maps to its tag;
  - exactly one tag line appears, before "panicked at";
  - a hook set beforehand still runs, which proves the chaining;
  - two entry points install once;
  - empty is absent;
  - all-or-nothing;
  - `environmental` and `transient` print their own tags.
- [x] `.github/scripts/classify_failures.py`:
  - in `classify()`, the last `^polyoxide-class=(\S+)\s*$` line wins, an unknown tag is `REAL`, and otherwise the existing regexes apply;
  - `parse_nextest_json` strips a trailing `#\d+` from test names.
- [x] `.github/scripts/tests/test_classify_failures.py` -- add tests, with the 27 kept unchanged:
  - a tag beats a matching regex, in both directions;
  - the last tag wins;
  - a tag in the middle of a line is ignored;
  - an unknown tag is `REAL`;
  - a tagged fixture parsed end to end;
  - a merge that promotes a `…#3` persistent transient, with a new fixture carrying the suffix.
- [x] `scripts/live_unwraps.py` and `scripts/live_unwraps.baseline.json` -- new.
  - The counts include `mod` files and ignore comments and opted-out lines.
  - It fails when a count rises, falls without the baseline being lowered, or belongs to a file missing from the baseline.
  - It fails when the classifier's pattern set differs from the frozen list.
  - **Test:** `.github/scripts/tests/test_live_unwraps.py` runs it on the real tree, plus cases for added and removed unwraps, a new file, and a new regex.
- [x] `scripts/gen_registry.py` -- `env_names()` also collects literal arguments of `load_env(`, `optional_env(` and `keychain(`, and refuses non-literals. Add tests.
- [x] `.github/scripts/tests/test_test_support_edges.py` -- from `cargo metadata`:
  - test-support's in-workspace dependencies are a subset of {core, venue};
  - every edge into it is a path-only dev-dependency;
  - core and venue never depend on it.
- [x] `CLAUDE.md` -- under the nightly section, one paragraph:
  - tags come first: `or_fail`, loaders, `environmental`, `transient`;
  - the regexes are a fallback for untagged logs, and `live_unwraps.py` counts down to zero;
  - remove the old "add a regex" guidance if any.

**Acceptance Criteria:**
- Given a test that calls `.or_fail` on an `Err` whose class is `Network`, when it fails, then its log carries `polyoxide-class=transient` and the classifier retries it.
- Given a failure that fails all three attempts, when `merge` runs, then it is promoted to `real`.
- Given an unset or empty secret, when a loader runs, then the log carries `polyoxide-class=auth-gated` and the failure is skipped.
- Given a PR adding an unwrap to a live test or a regex to the classifier, when CI runs, then the scripts job fails.

## Implementation Notes

## Spec Change Log

- **Mid-implementation amendment (Claude, as the user's delegate, 2026-10-08), found by the D2 investigation.**
  - **The tag map:** `Restricted` tags `environmental` only when `is_fault()` is false, and `real` otherwise. This is recorded as proposed spine amendment A2-1 in `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-2.md`. It supersedes the frozen Decision "Restricted → environmental" and Story 2.3's "class alone". Binance's 418 ban and WAF 403 are faults, so they stay `real`; a region block or a 451 is not, so it is `environmental`.
  - **The toolkit gains `fail(ctx, &E) -> !`** for `E: Classify + Debug + ?Sized`, because Story 2.5 needs it for match arms, borrowed errors and error chains.
  - **`UsdmWsError` reports a handshake 451 as a non-fault.**

## Review Triage Log

One layer (edge-case hunter), with 18 findings.

**Patched:**
- **Medium:** last-tag-wins lets a stale tag decide, including a `real` tag followed by an `environmental` one. A tag now counts only when it directly precedes the final `panicked at`.
- **Medium:** the regex freeze misses inline and in-function regexes, and a non-pattern table item reads as removed. This is the acceptance criterion's own claim.
- **Medium:** opt-out comments bypass the ratchet silently. Opt-out counts are now baselined.
- **Medium:** `.env` `set_var` races under multi-threaded `cargo test`, and a malformed line truncates loading silently.
- **Low:** the tag can join the end of an unterminated stdout line; the loader scan fails on a loader name inside a string or comment; `Result::unwrap` used as a path is not counted.

**Deferred** (deferred-work.md): `fail_classified` for `ClassifiedError` (S3); a Binance supervised `Stopped` caused by a 451 keeps no cause (Story 4.10).

**Rejected:**
- **A later non-chaining panic hook drops tags.** Low and unlikely, and the AC requires the hook design.
- **A loader used as a value or imported under an alias, and nested or `#[path]` `mod` files.** Low and unlikely in live tests.
- **A versioned dev-dependency on the toolkit is invisible to the edges test.** The package job's publish dry run already fails on it.
- **The implementation departs from the frozen "Restricted → environmental".** This is amendment A2-1, recorded in the Spec Change Log and in `spine-amendments/epic-2.md`.

## Verification

**Commands:**
- `cargo test -p polyoxide-test-support -j 4` -- expected: pass. Keep target dirs out of `/tmp`; the disk is about 95% full.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass, including the 27 existing classifier tests.
- `python3 scripts/live_unwraps.py` -- expected: exit 0.
- `python3 scripts/gen_registry.py --check && python3 scripts/publish_order.py check-manifests` -- expected: success.
- `cargo clippy -p polyoxide-test-support --all-targets -j 4 -- -D warnings` and `cargo doc -p polyoxide-test-support --no-deps` with `-D warnings` -- expected: clean.
