# cargo-semver-checks reports and scratch-crate builds

- Tool: cargo-semver-checks 0.51.0
- Toolchain: Rust 1.99.0

These are the versions `ci.yml` and `release.yml` pin, and
`test_api_removals.py` fails when the pins move without a recapture. All files were
captured on 2026-10-08 in a worktree at commit `a35a8ee`, plus the uncommitted Stories
1.6–1.8, which change no Rust source. Captured with stdout and stderr in one pipe, as
`scripts/api_removals.py` reads them, with `RUSTFLAGS` unset and `CARGO_BUILD_JOBS=4`.
The worktree's absolute path is replaced with `/home/runner/work/polyoxide/polyoxide`,
and the home directory with `/home/runner`; nothing else is edited. Every scratch edit
below was reverted, and its file checked against its sha256 afterwards.

- `clean.txt`, exit 0, on the unedited tree:

  ```
  cargo semver-checks --workspace --baseline-rev v0.38.1 --release-type patch --color never
  ```

- `mixed.txt`, exit 100, after four scratch edits:

  ```
  cargo semver-checks -p polyoxide-rtds -p polyoxide-sports --baseline-rev v0.38.1 --release-type patch --color never
  ```

  - `polyoxide_rtds::decode::decode_plain` made `pub(crate)`: `function_missing`.
  - `#[non_exhaustive]` on `polyoxide_rtds::payload::DisplayPoint`:
    `struct_marked_non_exhaustive`, a change that is not a removal.
  - `MatchUpdate::key` in polyoxide-sports made `pub(crate)`:
    `inherent_method_missing`, reported once for each path `MatchUpdate` is
    importable at.
  - `#[must_use]` on `SportsWsBuilder::new`: `inherent_method_must_use_added`, a
    minor-level lint that `--release-type patch` makes a failure.

- `twins.txt`, exit 100, with `pub fn limit` made `pub(crate)` on both of polyoxide-data's
  `ListTrades` (`src/api/trades.rs:80` and `src/v2/api/feeds.rs:132`). The tool prints the
  same item text twice; only the file tells them apart:

  ```
  cargo semver-checks -p polyoxide-data --baseline-rev v0.38.1 --release-type patch --color never
  ```

- `compile-dependency-error.txt` and `compile-unresolved-import.txt` are the scratch-crate
  build for polyoxide-sports with `test-server`, as `compile_hidden` runs it:

  ```
  cargo check --manifest-path target/api-removals/polyoxide-sports+test-server/Cargo.toml --target-dir target/api-removals/target --message-format json --color never
  ```

  - `compile-dependency-error.txt`: line 5 of `polyoxide-sports/src/lib.rs` replaced with
    `#![feature(never_type)]`, an error in the dependency at `src/lib.rs:5`, the line where
    the scratch crate imports its first path.
  - `compile-unresolved-import.txt`: the listing plus `polyoxide_sports::fixtures::NOT_THERE`,
    which fails at line 13 of the scratch crate's own `src/lib.rs`.
