---
paths:
  - "**/tests/live_*.rs"
  - "**/tests/failure_tags*.rs"
  - "polyoxide-test-support/**"
  - "scripts/gen_registry.py"
  - ".github/workflows/nightly-*.yml"
  - ".github/scripts/classify_failures.py"
  - ".github/scripts/tests/test_{classify_failures,live_registry,live_unwraps}.py"
  - "scripts/live_unwraps*"
  - ".env.example"
---

# Live tests and the nightly

Live suites hit the real upstream APIs: `tests/live_<module>[_<suite>].rs` in each venue crate,
the CLI's `tests/live_api.rs` and the umbrella's `tests/live_venue.rs` (the venue-neutral example
against each venue). All are `#[ignore]`d so CI skips them; run with `-- --ignored`.

`nightly-behavioral.yml` runs them, one job per crate and suite (the generated list in
CLAUDE.md), each built with `--no-default-features` and its targets' declared features alone, and
each job's `env:` holding only the secrets its tests read. An unset repository secret arrives as
`""`, which the loaders treat as unset. Kalshi's `live_predictions` and `live_predictions_venue`
read production unsigned, so every nightly run reads real markets; its signed suites run against
the demo host (`kalshi.md`).

## Tags decide

A live test that fails through `polyoxide-test-support` prints `polyoxide-class=<tag>` alone on a
stderr line just before it panics, and that line is all `.github/scripts/classify_failures.py`
reads. Only the tag just above the log's final `panicked at` report decides, since one anywhere
else belongs to a panic the test caught or a spawned task raised; a `real` or unknown tag
anywhere makes the failure `real`, and so does no tag at all. Nothing reads the panic text.

- `.or_fail(ctx)` (`ResultExt`, and `ClassifiedResultExt` for a venue trait's `ClassifiedError`),
  and `fail(ctx, &err)` (`fail_classified` for a `ClassifiedError`) for an error in a match arm or
  a source chain, take the tag from the error's `Class` and `is_fault()`: `Network`, `Unavailable`
  and `RateLimited` are `transient`; `Restricted` is `environmental` when it is not a fault (a 451
  region block) and `real` when it is (a 418 ban the client earned); every other class is `real`.
- The credential loaders (`load_env`, `optional_env`, `keychain`) count an empty value as absent
  and fail through `Missing::or_auth_gated()` as `auth-gated`.
- `environmental(reason)` and `transient(reason)` tag what no error value carries.
- Wrap a foreign error in the venue's own error first (`ApiError::Network`,
  `UsdmWsError::from(WsError::from(..))`, `RtdsError::from(WsError::from(..))`,
  `ClobWsError::from(WsError::from(..))`, `SportsError::Transport { source: WsError::from(..) }`),
  and a raw socket's close frame likewise, so it takes its tag from the socket table.

| Tag | Means | The nightly |
|---|---|---|
| `auth-gated` | a credential loader found a secret absent or empty | skips it silently |
| `environmental` | the world can't give signal now (the sports feed with no live match, a quiet RTDS or perps feed, no market satisfying an order test's precondition), or a `Restricted` non-fault such as Binance refusing a US runner's location with a 451 | logs and skips it |
| `transient` | `Network`, `Unavailable` or `RateLimited` (HTTP 408/425/429/5xx, connection refused, timeouts, DNS, a dropped socket, close codes 1000/1001/1006/1011–1013), or `transient(reason)`, which socket tests call where a bare stream ends without a close code | retries up to 2x (`cargo nextest --retries 2`), then files it as real, unless the retry is environmental or auth-gated, whose verdict it then takes |
| `real` | everything else, every untagged failure included | files or updates the `nightly-behavioral` tracking issue |

- Only a run on `main` touches that issue: the step that files, comments on and closes it is
  guarded by `github.ref == 'refs/heads/main'`. A dispatch on another branch (an integration
  branch before it merges) classifies and uploads its reports and leaves main's issue alone; a
  step guarded by `github.ref != 'refs/heads/main'` writes the combined report to the run's summary
  and fails the run on any real failure, since the live jobs pass whatever their tests did.
- nextest suffixes a retried test's name with `#<attempt>`; the classifier strips it, or `merge`
  would miss every persistent transient.
- The behavioural workflow also runs Saturday and Sunday at 18:30 UTC (`30 18 * * 6,0`): the
  sports feed carries only what is live, and 06:00 UTC misses North American leagues and weekend
  soccer. That run covers every live job, so its clean result may close the issue.

## `scripts/live_unwraps.py`

The regexes that once guessed a failure's kind from its panic text are deleted, and this script
keeps it that way. It freezes every regex the classifier's source builds, read from its AST (what
is left parses the tag line, the panic report and nextest's retry suffix). It holds a per-file
count, at zero, of `.unwrap()`/`.expect(` and `.unwrap_err()`/`.expect_err(` (and
`Result::unwrap`-style paths) in live tests, and of bare failure sites: every `panic!`,
`unreachable!`, `todo!` and `unimplemented!`, and every `assert!`-family macro whose message
interpolates or passes a value named for an error or a status.

A line that must keep one, such as a parse of test data or an assertion on the response, ends
with `// live-unwraps: <reason>`. It fails untagged, so it files as `real`, and moves to a
per-file `opted_out` count that may not rise without a hand edit a reviewer sees.

Each regex-era case is a row of `.github/scripts/tests/test_classify_failures.py`'s tag table, and
each row naming an error the tests can build has a Rust twin in a failure-tags suite asserting the
tag that error prints: `polyoxide-test-support`'s `tests/failure_tags.rs` (core's `ApiError`),
`polyoxide-binance`'s `tests/failure_tags_usdm.rs`, and `polyoxide-polymarket`'s
`tests/failure_tags_sports.rs` and `tests/failure_tags_rtds.rs`.

## Reading the environment

A live target reads the environment only through the loaders, which take their env names as
string literals: `gen_registry.py`'s `env_names()` reads those calls for the secrets check, and
`direct_env_reads()` refuses any `std::env` or `env::` path, `env!`/`option_env!`, `dotenvy` or a
library `from_env()` in a live target, whose names that check could not see. The loaders read a
`.env` file into a private map behind the environment, never with `set_var`, and warn, naming the
file and line, of any line that does not parse.

To enable CLOB and relay's auth-gated tests, set the `POLYMARKET_*`, `BUILDER_*` and `RELAYER_*`
repo secrets: the loaders find them and the tests start contributing signal, with no change to the
classifier. Relay's credentialed tests fail as `auth-gated` without their secrets, like clob's;
they no longer pass silently.
