---
title: 'Stories 2.1 and 2.2: The classification vocabulary, and Classify for today''s error enums'
type: 'feature'
created: '2026-10-08'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '8aac730c24553fcb24f8c50ec0856f8a3f6cc33f'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-2-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Each venue error type answers "should I retry?" its own way, and the nightly classifier guesses from panic text. Consumers write their retry and alerting logic once per crate.

**Approach:**
- **Story 2.1.** A new crate, `polyoxide-venue`, holds the vocabulary from AD-3 and AD-15:
  - `Class`, `Classify` and `ClassifiedError`;
  - `Secret<T>`;
  - the status-to-class map and the socket close-code map;
  - the one `Retry-After` parser.
- **Story 2.2.** Every public error type in the workspace implements `Classify`. No variant, `Debug` or `Display` output changes (AD-16).

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08), filling gaps the spine leaves open:**
- **One bundle, one review layer.** The table tests pin the behaviour.
- **Status outside the error range.** `class_for_status(u16) -> Option<Class>` returns `None` for statuses outside 4xx and 5xx. Each impl picks the fallback:
  - status `0` means no response (clob's service-failure path) → `Network`;
  - a 1xx, 2xx or 3xx status reported as an error → `Decode`.
- **`code` holds the venue's code only, never the HTTP status.** Use `None` when the venue sent none. Numeric codes are written in decimal.
- **The `Retry-After` parser:**
  - it trims, parses the value as decimal seconds, and caps it at the clamp it is given;
  - it returns `None` for zero, negative, NaN, infinity, an HTTP-date, or garbage;
  - a huge finite value comes back as the clamp, and never panics;
  - `retry_delay(requested, computed)` returns `max(requested, computed)`, so the parser only ever lengthens the client's own backoff.

  The existing call sites keep their own parsers until Story 3.10.
- **Variants that are ambiguous today:**
  - `ApiError::Validation` → `VenueRefusal`. It mixes a server 400 with clob's local validation, and both tag `real`; Epic 3's reshape splits them.
  - `KeychainError` → `InvalidRequest`.
  - `RelayError::Api(String)` → `VenueRefusal`, until Story 3.5's DRIFT R7 reshape.
  - `PerpsWsError::Refused` → `RateLimited{None}` when every refusal is `message_rate_limited`, else `VenueRefusal{code: first reason}`.
  - `RtdsError::Server{status}` follows the status rule, because no DFR row overrides it.
  - Binance's socket handshake 403 → `Restricted`, applying D14 at the same WAF edge.
- **The socket table:**
  - `tungstenite` errors map per AD-15. `WriteBufferFull`, `Utf8` and any unknown variant are `Network`.
  - Close codes not listed in AD-15 (none, 1005, 1014, 1015, 3000–3999) are `Network`.
  - The close-code and handshake-status maps live in venue as pure functions. The per-crate match over `tungstenite::Error` stays in each socket crate until Story 4.3's `impl_ws_classification!`.
- **"Every public error type" includes the inner and auxiliary ones,** so the guard needs no exclusion list: `V2Error`, `VenueError`, `BurstCapacityExceeded`, `ParseTickSizeError`, both `UnknownVariant`s, `InvalidSymbol` and `InvalidStreamName`.
- **Inherent `is_retriable`/`retry_after` methods stay.** Where they disagree with the trait's answer, the table tests pin both answers. Epic 3's reshape removes the inherent ones.
- **Dependencies are the minimum used.** `polyoxide-venue` takes only what it uses now: `thiserror`, and `serde` only if `Class` needs it. AD-3's list is an upper bound; `rust_decimal`, `futures-core` and `dynosaur` arrive with the first item that needs them.

## Boundaries & Constraints

**Always:**
- `ClassifiedError` must not implement `Classify`; otherwise the blanket `From<E: Classify>` overlaps `From<T> for T`. Its source is `Box<dyn Error + Send + Sync + 'static>`.
- `is_fault()` is false only when the venue answered as designed: FAK and FOK kills, and Binance's region block.
- Gate each impl and each assertion exactly as its type is gated (`keychain`, the `ws` features), so `cargo hack --each-feature` passes.
- Register the new crate:
  - a `members` entry and `[workspace.dependencies] polyoxide-venue = { path, version = "0.38.1" }`;
  - workspace-inherited metadata with a `description`;
  - `[package.metadata.polyoxide] readme = …`, with no venue or products;
  - run `python3 scripts/gen_registry.py --write`.
- Core and every implementing crate depend on `polyoxide-venue` directly (AD-1).

**Never:**
- Change a variant, a `Debug` or `Display` output, or an inherent method.
- Switch an existing `Retry-After` call site; that is Story 3.10.
- Remove a public item. The removal gate must still pass.
- Add `reqwest`, `tokio-tungstenite` or `polyoxide-core` to venue's dependencies.

</frozen-after-approval>

## Code Map

- **The spine:**
  - AD-15 (`ARCHITECTURE-SPINE.md:377-411`) holds the classes, the status map, the `is_fault` and `is_retriable` rules and the socket table.
  - AD-3 (:94-114) lists what venue holds and may depend on.
  - AD-1 (:69-79): core depends on venue.
  - AD-2 (:81-92) is the credential-free fence for rtds and sports.
- **The `Retry-After` parsers today:**
  - core `client.rs:145-155`: f64, finite and greater than 0, clamped by `max_backoff_ms`;
  - binance `error.rs:128-137`: trimmed, finite and greater than 0, capped at 3 days;
  - data `v2/error.rs:94-97`: finite and at least 0, no clamp. It **panics on huge values**, because `Duration::from_secs_f64` overflows. Pin huge values in venue's table, but leave data's code alone;
  - perps `error.rs:64-66`: parsed as `u64`.
- **Retriable-status rules:** core `error.rs:93`, perps `error.rs:77`, binance `error.rs:102`.
- **The enums and their proposed classes:**
  - **core `ApiError`** (`polyoxide-core/src/error.rs:5-37`):
    - `Api{status}`: the status rule, with the fallback above;
    - `Authentication` → `Unauthorized`;
    - `Validation` → `VenueRefusal`;
    - `RateLimit` → `RateLimited{None}`;
    - `Timeout` → `Unavailable` (built only from 408);
    - `Network(reqwest)`: `is_builder` → `InvalidRequest`, `is_decode` → `Decode`, otherwise `Network`;
    - `Serialization` → `Decode`;
    - `Url` → `InvalidRequest`.
  - **core `KeychainError`** (`keychain.rs:13`, under the `keychain` feature).
  - **`GammaError`** (gamma `error.rs:6`): `Api` delegates.
  - **`DataApiError`** (data `error.rs:11`):
    - `Api` delegates;
    - `V2(V2Error{status, code, retryable, retry_after})` follows the status rule, with `code` from `code.as_str()` and `retry_after` from its field (zero filtered out);
    - `Pagination` → `Decode`.
  - **`ClobError`** (clob `error.rs:14`):
    - `Api` delegates;
    - `Crypto` and `Alloy` → `InvalidRequest`;
    - `InvalidTickSize` → `Decode`;
    - `FakUnmatched` and `FokUnfilled` → `VenueRefusal{None}`, with `is_fault` false;
    - `BurstCapacityExceeded` → `InvalidRequest`.
  - **`RelayError`** (relay `error.rs:8`):
    - `Reqwest` uses the reqwest rule;
    - `UrlParse`, `Signer` and `MissingSigner` → `InvalidRequest`;
    - `SerdeJson` → `Decode`;
    - `RateLimit` → `RateLimited`;
    - `Core` delegates;
    - `Api(String)` → `VenueRefusal`.
  - **`PerpsError`** (perps `error.rs:12`): `Api` delegates; `Venue(VenueError)` follows the status rule, with `code = error` and `retry_after` from its field.
  - **`BinanceError`** (binance `error.rs:14`):
    - `Api` delegates;
    - `Venue{status, code}` follows the status rule, with the code in decimal;
    - `RateLimited` → `RateLimited{retry_after}`;
    - `IpBanned` → `Restricted`, with `retry_after` still set;
    - `RegionBlocked` → `Restricted`, with `is_fault` false;
    - `Forbidden` → `Restricted` (D14).
  - **`PerpsWsError`** (perps `ws/error.rs:37`):
    - `Connection` → the socket table;
    - `ConnectionClosed` and `Stalled` → `Network`;
    - `Refused` (see Decisions);
    - `Response`, `Frame` and `Unrecognised` → `Decode`;
    - `EmptySubscription` and `Stopped` → `InvalidRequest`.
  - **`UsdmWsError`** (binance `usdm/ws/error.rs:24`):
    - `Connect` → the socket table, with handshake 403 → `Restricted`;
    - `ConnectTimeout` and `NoAnswer` → `Network`;
    - `Closed{code}` → the close-code map;
    - `Refused{code}` → `VenueRefusal`;
    - `Response` and `Frame` → `Decode`;
    - `TooManyStreams`, `WrongPath` and `Stopped` → `InvalidRequest`.
  - **`SportsError`** (sports `error.rs:27`):
    - `Connect` and `Transport` → the socket table;
    - `ConnectTimeout` and `Stale` → `Network`;
    - `Closed{code}` → the close-code map;
    - `Decode` → `Decode`.
  - **`RtdsError`** (rtds `error.rs:12`):
    - `Connection` → the socket table;
    - `Json` and `Precision` → `Decode`;
    - `ConnectionClosed` and `Stalled` → `Network`;
    - `Url` and `EmptySubscription` → `InvalidRequest`;
    - `Server{status}` follows the status rule.
  - **clob `WebSocketError`** (`ws/error.rs:5`):
    - `Connection` → the socket table;
    - `Json` → `Decode`;
    - `ConnectionClosed` and `ConnectTimeout` → `Network`;
    - `Authentication` → `Unauthorized`;
    - `InvalidMessage`, `Url` and `MembershipClosed` → `InvalidRequest`.
  - **`PolymarketError`** (`polyoxide/src/lib.rs:140`): each variant delegates; `Config` → `InvalidRequest`.
- **Auxiliary types:**
  - `BurstCapacityExceeded` (core `signer_limit.rs:234`) → `InvalidRequest`;
  - `ParseTickSizeError` (clob `types.rs:10`) → `InvalidRequest`;
  - `V2Error` (data `v2/error.rs:55`) and `VenueError` (perps `error.rs:32`) → the status rule;
  - `UnknownVariant` (perps `types.rs:29`, binance `usdm/types.rs:83`) → `Decode`;
  - `InvalidSymbol` (`usdm/types.rs:35`) and `InvalidStreamName` (`usdm/ws/stream.rs:142`) → `InvalidRequest`.
- **Dependencies:** thiserror 2.0 and serde 1.0 are in `[workspace.dependencies]`. dynosaur and futures-core are absent, and stay absent here.
- **Generated text:** the rtds and sports `notes` in their `Cargo.toml` (`[package.metadata.polyoxide]`) feed the CLAUDE.md graph. The hand-written rules that need rewriting are `CLAUDE.md` ~466 (sports: "depends on nothing in the workspace") and ~474 (rtds: "nothing else in the workspace (not even core)").

## Tasks & Acceptance

**Execution:**
- [x] `polyoxide-venue/` -- new crate: `Cargo.toml`, `README.md` (a doctest-safe example) and `src/lib.rs`, with modules for `class`, `status`, `retry_after`, `secret` and `socket`.
  - `Class`: `#[non_exhaustive]`, with the 8 variants, where `code: Option<Arc<str>>` and `retry_after: Option<Duration>`.
  - `Classify`: `class()`, `is_fault()` (defaulting to `true`), `retry_after()` (defaulting to the class's) and the provided `is_retriable()`.
  - `ClassifiedError`.
  - `Secret<T>`: a redacted `Debug`, `expose()`, and `Clone` where `T: Clone`.
  - `class_for_status`.
  - `class_for_close_code(Option<u16>)`.
  - `parse_retry_after(&str, clamp)` and `retry_delay`.

  Each has rustdoc. There are no intra-doc links to private items.
- [x] Root `Cargo.toml` -- add the member and the workspace dependency.
- [x] `polyoxide-venue/tests/` or unit tests -- a table test pinning every status row (100, 200, 0, 400, 401, 403, 404, 408, 418, 425, 429, 451, 500, 503, 599, 600), every close code in AD-15 plus the unlisted ones, and parser cases: `"0"`, `"-1"`, `"NaN"`, `"inf"`, `"1e400"`, `"1e20"` (which clamps), `"1.5"`, `" 2 "`, an HTTP-date, `""` and `"abc"`.
- [x] Each crate's `error.rs` (and its `ws/error.rs`) -- `impl polyoxide_venue::Classify` per the Code Map. Each module gets a table test covering every variant: class, `is_fault`, `retry_after`, and, where an inherent method exists, both `is_retriable` answers.
- [x] Each crate's `lib.rs` -- `const _: fn() = || { fn is<T: polyoxide_venue::Classify>() {} is::<…>(); … };`, listing that crate's public error types and gated like them.
- [x] `.github/scripts/tests/test_classify_coverage.py` -- new. It sweeps `pub (enum|struct) \w*Error\b`, plus `UnknownVariant`, `InvalidSymbol` and `InvalidStreamName`, over `polyoxide*/src`, and fails when a name is absent from its crate's assertion. A test proves it catches a new type.
- [x] `polyoxide-rtds/Cargo.toml`, `polyoxide-sports/Cargo.toml` -- each gains only the `polyoxide-venue` dependency, and their metadata `notes` are rewritten. Then `CLAUDE.md` ~466 and ~474 are rewritten too: rtds and sports depend on `polyoxide-venue` alone, and still not on core, reqwest or alloy. Run `gen_registry.py --write`.
- [x] `CLAUDE.md` -- under the error-hierarchy paragraph, one paragraph says every error implements `polyoxide_venue::Classify` and that the class decides retriability for callers. Where an inherent `is_retriable` still exists, it may disagree until Epic 3.

**Acceptance Criteria:**
- Given any public error type, when it is built, then it implements `Classify`, and a missing impl fails the build or the coverage test.
- Given AD-15's status and close-code rows, when the table tests run, then each maps as specified, and FAK and FOK kills are `VenueRefusal` with `is_fault() == false`.
- Given the workspace, when `cargo tree -p polyoxide-rtds` and `-p polyoxide-sports` run, then neither contains reqwest, alloy or polyoxide-core.
- Given the removal gate and the package job, when they run, then they pass, and venue is a new name (one of at most five).

## Implementation Notes

All local runs used `-j 4`, with target dirs under the worktree's `target/`. Nothing was committed.

- **`polyoxide-venue` has no dependencies at all.** `ClassifiedError` is hand-written, not `thiserror`: it displays as its source does and continues the source chain from there, and thiserror's `transparent` needs a single field. `Class` needs no serde yet.
- **Choices the spec left open:**
  - `Classify` has `std::error::Error` as a supertrait, so `From<E: Classify + Send + Sync + 'static>` can box the error.
  - `ClassifiedError` keeps `class` and `source` public. It also records `is_fault` and `retry_after` at conversion, behind accessors, because a Binance ban's wait and a FAK kill's non-fault are not derivable from the class. `ClassifiedError::new(class, source)` covers an error that is not `Classify`.
  - `Class` gains `is_retriable`, `retry_after`, `code`, `with_code` and `with_retry_after`. `retry_after()` is never `Some(ZERO)`: `with_retry_after` and `Class::retry_after` drop a zero, and the `V2Error`, `VenueError` and Binance overrides do too. The spec names that filter for data only.
  - `class_for_handshake_status(u16)` is the status rule, with a non-error status as `Decode`.
  - `parse_retry_after` uses `Duration::try_from_secs_f64`, so `1e300` against `Duration::MAX` clamps rather than panicking. `1e400` parses to infinity and is `None`, and `0.0000000004` rounds to zero and is `None`, both as Binance's parser already does.
  - `PerpsWsError::Refused` → `VenueRefusal`, with its code taken from the first reason other than `message_rate_limited` and the client's synthetic `err` (now the `NO_REASON` constant). A mixed refusal's first reason can be the rate limit, which is not why it is a refusal.
  - core gains `pub fn polyoxide_core::error::classify_reqwest`, the reqwest rule, which `RelayError::Reqwest` shares.
  - Each auxiliary type's impl sits beside the type (`signer_limit.rs`, `types.rs`, `stream.rs`), each with its own test.
- **The mutant ledger.** The clob impl sits before the test module and uses fully qualified paths, so the cited lines 89, 92 and 110–113 do not move. Its six cited tests moved 41 lines down. `docs/MUTANTS.md` and `test_mutants_ledger.py` cite their new lines. Rows (e) and (e), case were proved again: under the `&&`→`||` mutant, `test_classify_requires_both_tokens` fails; under `to_string()`, the four lib tests and two `mock_api` tests fail. All pass once the line is restored.
- **The coverage test** also sweeps any type deriving `Error`, which is how `BurstCapacityExceeded` is found, and skips `polyoxide-venue` (its `ClassifiedError` must not implement the trait). A real-tree mutation, deleting `is::<BurstCapacityExceeded>()`, fails it naming the type.
- **Also changed:** `test_publish_order.py::test_real_workspace_order` now expects `polyoxide-venue` first and core second. CLAUDE.md's Retriability paragraph no longer calls the inherent `is_retriable` "the canonical classifier".
- **Verification:**
  - fmt and clippy `-D warnings` are clean, and so is `cargo doc` with `-D warnings`;
  - `cargo test --workspace --all-features` passes 2,076 tests with 0 failures (167 ignored), and the doctests pass;
  - `cargo tree -e normal` for rtds and sports shows only `polyoxide-venue` from the workspace, and no reqwest, alloy, core, governor, hmac, sha2 or keyring;
  - `gen_registry --check`, `check-manifests` and the publish dry run (13 crates) succeed;
  - 662 scripts tests pass;
  - `cargo hack check --each-feature --no-dev-deps --ignore-private` ran 55 checks, exit 0. It found one unused import in the umbrella with every feature off, which is fixed;
  - MSRV 1.91 `cargo check --workspace --all-features` passes;
  - `api_removals.py check --baseline v0.38.1` on 1.99.0 exits 0, reporting "no semver update required" for all 11 baseline crates and skipping venue as new.
- **Not run:** the Python bindings job (`polyoxide-py` pytest), and the live suites.
- **Review loop 1 patches:**
  - `classify_reqwest` checks a failure in transit first (a timeout, a connect, `is_body`, or an `io::Error` or body-kind reqwest error in the source chain) → `Network`, because reqwest labels a body that broke off mid-read `is_decode`. Then a carried status follows the status rule. New table rows: a cut-off body, a timeout, and `error_for_status` 503 and 404.
  - `ApiError::Api { status: 0 }` → `VenueRefusal { code: None }`, amending the spec's `Network`: clob wraps every Gamma failure in it.
  - A 451 is not a fault in `ApiError::Api`, `VenueError` (and `PerpsError::Venue`), `V2Error` (and `DataApiError::V2`) and `BinanceError::Venue`.
  - `ClassifiedError::from` drops a zero wait.
  - Both `UnknownVariant`s → `InvalidRequest`. Only caller-side `FromStr` builds one: Binance's server-side stream parse maps it into `InvalidStreamName` and then `Frame`, and perps' push parse discards it as `Frame::Unknown`.
  - The perps and data tables pin the inherent `retry_after()` beside the trait's (they disagree on `Retry-After: 0`).
  - `ApiError::is_retriable`'s rustdoc points to `Classify::is_retriable`.
  - The coverage test also collects `impl (std|core)::error::Error for X`, sweeps `polyoxide-venue` with only `ClassifiedError` exempt, and reads sources as UTF-8. Its new tests fail with each fix reverted.

## Spec Change Log

- **Decision amended in review (Claude, as the user's delegate, 2026-10-08).** Status `0` now classes as `VenueRefusal { code: None }`, not `Network`. `ClobError::service` builds status 0 for every Gamma failure, including 4xx responses, decode failures and local builder failures, so `Network` would make retry loops spin. `VenueRefusal` matches today's inherent non-retriable answer. The frozen Decisions line above is superseded by this entry.

## Review Triage Log

One layer (edge-case hunter), with 22 findings.

**Patched:**
- **Medium:** `classify_reqwest` classes a body-read timeout or reset as `Decode`, and ignores the status a reqwest error carries.
- **Medium:** status 0 as `Network` (see the Spec Change Log).
- **Low:**
  - 451 `is_fault` is inconsistent across impls;
  - `ClassifiedError::from` can store `Some(0)`;
  - binance's `UnknownVariant` (from caller input) is `Decode`;
  - perps' synthetic `"err"` code;
  - the inherent-versus-trait `retry_after` answers are unpinned;
  - `ApiError::is_retriable`'s rustdoc still says it is canonical;
  - the coverage sweep misses hand-written `impl Error`, skips the venue crate, and reads files with the locale encoding.

**Deferred** to Story 4.3 (in deferred-work.md): the handshake `Retry-After` is dropped in the socket impls, and clob's zero-address DNS miss is `InvalidRequest`.

**Rejected:**
- **A rustls certificate failure arrives as `Io` and is classed `Network`.** It follows AD-15 as ratified: the spine's memlog records that rustls 0.23.38 surfaces certificate failures as `Error::Io`. Refining it is Story 4.3's job.
- **Binance's 1008 "Too many requests" is `VenueRefusal`.** AD-15's close-code table puts 1008 under `VenueRefusal`.
- **RTDS `Server{429/5xx}` is retriable by class while `recovery()` is fatal.** AD-15 separates classification from recovery on purpose.
- **`RelayError::Api` for 5xx and 429 is `VenueRefusal`.** That is a spec decision until DRIFT R7 in Story 3.5.
- **Same-named error types in different modules of one crate satisfy the coverage list together.** Low and unlikely, and fixing it means keying the check by module path.
- **Data's `ErrorCode::Unknown` gives the code "unknown".** The original string is not retained, and keeping it needs a variant change (AD-16).
- **The perps refusal code is the first reason other than the rate limit, rather than the first reason.** The implementer's reading avoids a mixed refusal carrying the rate-limit identifier as its code.

## Verification

**Commands:**
- `cargo test -p polyoxide-venue -j 4` -- expected: pass.
- `cargo test --workspace --all-features --lib -j 4` -- expected: pass. Keep target dirs out of `/tmp`; the disk is about 94% full.
- `cargo clippy --all-targets --all-features -j 4 -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace -j 4` -- expected: clean.
- `cargo tree -p polyoxide-rtds -e normal | grep -E 'reqwest|alloy|polyoxide-core'` -- expected: no output. Same for sports.
- `python3 scripts/gen_registry.py --check && python3 scripts/publish_order.py check-manifests && cargo publish --workspace --dry-run --no-verify --locked --allow-dirty` -- expected: success.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass.
- `env -u RUSTFLAGS PATH=$PWD/target/rust-1.99.0/prefix/bin:$HOME/.cargo/bin:$PATH CARGO_BUILD_JOBS=4 python3 scripts/api_removals.py check --baseline v0.38.1` -- expected: exit 0.
