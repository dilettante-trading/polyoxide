# Epic 2 Context: Nightly failures classified by error class, one test toolkit

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Stage S1, third in S1 order, after Epic 1's publish-order script and generator. Nightly sorts failures with regexes over panic text today, so every venue adds regexes and a partial credential set is filed as a fault. This epic adds `polyoxide-venue`'s classification vocabulary, implemented over every existing error enum without changing them, and `polyoxide-test-support`, which prints a class tag at the failure site. `classify_failures.py` reads the tags, every live suite migrates, the regex table is deleted, and the agreement, fixture, soak and capture helpers (inventory T1–T9) end up existing once. The classifier has to stop depending on error text before Epics 3 and 4 reshape the enums.

## Stories

- Story 2.1: The classification vocabulary
- Story 2.2: Classify today's error enums
- Story 2.3: The test toolkit crate and the failure-tag reporter
- Story 2.4: The classifier reads tags
- Story 2.5: Migrate the HTTP live suites
- Story 2.6: Migrate the socket live suites
- Story 2.7: Retire the regex fallback
- Story 2.8: Shared agreement helpers, adopted by data v2 and perps
- Story 2.9: Gamma and Binance on the shared agreement helpers
- Story 2.10: Shared soak harness and capture helpers

## Requirements & Constraints

- **No error shape changes.** Variants, `Debug` and `Display` stay as they are. A compile-time assertion lists every public error type and fails the build if one lacks `Classify`.
- **Tags and their nightly actions:**
  - `auth-gated` (a credential loader) is skipped silently.
  - `environmental` (`Restricted`, or `environmental(reason)`) is logged and skipped.
  - `transient` (`Network`, `Unavailable`, `RateLimited`, or `transient(reason)`) is retried twice, and `merge` promotes a persistent one to `real`.
  - `real` (everything else) files the issue.
- **Precedence.** The last `polyoxide-class=` line wins, then the regex table, then `real`.
- **The baseline only shrinks.** CI fails when a live test unwraps a polyoxide `Result` without `or_fail`, or when a regex is added. The regex table is deleted once `scripts/live_unwraps.py`'s baseline is empty.
- **Credential loaders:**
  - An empty value counts as absent, because an unset repository secret arrives as `""`. This closes a deferred bug in which a partial secret set reaches `Account::from_env()` and fails as `real`.
  - The env names a test passes must equal its target's declared `secrets`. The existing `POLYMARKET_*`, `BUILDER_*` and `RELAYER_*` names keep working.
- **Hooks.** Every entry point installs its hook through a chained `Once`. A test must prove this under nextest's process-per-test model.
- **Test custody:**
  - Moves keep test names, assertions and per-suite counts, and each PR reports the counts before and after.
  - A consolidated helper is a superset of the forks it replaces.
  - Re-run `docs/MUTANTS.md` after each move.
  - The 27 `classify_failures` tests pass unchanged through 2.4. In 2.7 each becomes one tag-table test.
- **Soaks.** The soaks still detect `Retriable status 429` at `WARN` under the `polyoxide_core` target prefix.
- **Standing rules.** The CLAUDE.md rule that rtds and sports "depend on nothing in-workspace" is rewritten in 2.2. The `AUTH_GATED_RE` and nightly-classification text is rewritten in 2.7. Generated regions change only through `gen_registry.py`.
- **Gates.** New crates need `[package.metadata.polyoxide]` and must pass `python3 scripts/gen_registry.py --check`. A removed public item goes in `docs/s1-removals.md`. Rustdoc runs with `-D warnings`, so `pub` docs never link `pub(crate)` items.

## Technical Decisions

- **`polyoxide-venue`** depends only on rust_decimal, serde, thiserror, futures-core and dynosaur, so rtds and sports stay free of HTTP and signing dependencies. It exports:
  - `#[non_exhaustive] Class`, whose `code` fields are `Option<Arc<str>>` with numbers rendered in decimal;
  - `Classify`, with `class()`, `is_fault()` and `retry_after()`, plus a provided `is_retriable()` (Network, Unavailable and RateLimited);
  - `ClassifiedError { class, source }`, with `From<E: Classify>`;
  - `Secret<T>`, whose `Debug` is redacted;
  - the status-to-class function;
  - the one `Retry-After` parser. Its clamp is a parameter, and it may only lengthen the client's own backoff.
- **The retriable-status rule answers callers only.** The send loop's retry set, and moving venues onto the parser (DRIFT R4), belong to Epic 3.
- **Status before body:**
  - 401 and 403 → `Unauthorized`;
  - 418 and 451 → `Restricted`;
  - 429 → `RateLimited`;
  - 408, 425 and 5xx → `Unavailable`;
  - any other 4xx → `VenueRefusal`.
- **Classes no status produces.** `InvalidRequest` is client-side misuse, such as validation, a bad URL or use after close. `Network` means no response. `Decode` is a 2xx body that does not parse.
- **Overrides and faults.**
  - A status is overridden only through a DFR row: Binance's WAF 403 maps to `Restricted` (D14).
  - FAK and FOK kills map to `VenueRefusal` with `is_fault() == false`, and `classify_order_kill` is unchanged (D15).
  - A server-sent `retryable` flag (data v2) is surfaced, not obeyed.
- **Socket errors** follow the table Epic 4 centralises. Io, ConnectionClosed and close codes 1000/1001/1006/1011–1013 are `Network`. Close codes 4000–4999 and the other protocol close codes are `VenueRefusal`. Misuse such as Url, Tls or AlreadyClosed is `InvalidRequest`. A handshake status follows the status rule. Reconnect behaviour does not change (D19).
- **`polyoxide-test-support`:**
  - It is `publish = false` and depends only on `polyoxide-core` (with keychain) and `polyoxide-venue`, never on a venue crate.
  - Crates take it only as a path-only dev-dependency. Helpers that inline unit tests need stay in their crate.
  - `ResultExt::or_fail(ctx)` prints `polyoxide-class=<tag>` to stderr, with the tag taken from the class alone, then panics with `ctx` and the error.
- **Live-suite mapping.** Binance's 451, "no qualifying market" and sports' "legitimately time out" call `environmental(reason)`. A stream that ends without a close code calls `transient(reason)`.
- **Helpers (T1–T8)** move into test-support, and each one must cover every fork it replaces:
  - the synthesiser keeps perps' `$ref`-nullable, enum and example handling and `OBSERVED_EXTRA`;
  - the allow-lists keep gamma's dotted paths, `(key, reason)` tuples and array-length assertion, plus a direction-2 top-level check that skips `IGNORED`.

  The soaks use the helpers without `#[path]` includes.
- **`scripts/capture_common.py` (T9).** Its HTTP `get` and WebSocket client take per-request headers from a function the caller supplies, so signing stays in each venue's script.
- **The secrets check.** The loader calls replace `gen_registry.py`'s static env-name scan (`env_names()`/`target_source()`).

## Cross-Story Dependencies

- **Prerequisite.** Epic 1, including the removal gate from 1.7.
- **Order.** 2.1 → 2.2 → 2.3, because the reporter needs `Classify`. Then 2.4, then 2.5 and 2.6, then 2.7, which needs an empty baseline.
- **Helper stories.** 2.8 comes before 2.9, and 2.10 needs 2.3's crate.
- **Epics 3 and 4 start only after 2.6.** 2.7–2.10 may overlap them.
- **Downstream.** Epic 3 reshapes the HTTP enums onto 2.1's parser and status rule. Epic 4 adds `impl_ws_classification!` and reshapes the socket enums.
