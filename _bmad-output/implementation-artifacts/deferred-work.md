# Deferred work

- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-re-baseline-on-current-main.md`
  summary: Gamma's generic `null_as_empty<T>` (`polyoxide-gamma/src/types.rs:119`, v0.38.0) duplicates the older `deserialize_users` (`polyoxide-gamma/src/api/user.rs:100`); one should go.
  evidence: Both bodies are `Option<Vec<_>>::deserialize(..)?.unwrap_or_default()`; `null_as_empty` arrived in 63ddbb1. No duplication-inventory row covers gamma-internal serde helpers, so no restructure story removes it.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-re-baseline-on-current-main.md`
  summary: The gamma–sports `game_id` join uses three types (`i64` setters, `u64` on `Event` and sports `MatchUpdate`, `String` on `Market`), so a caller casts `u64` to `i64` to filter `/events` by the id sports sends.
  evidence: `polyoxide-gamma/src/api/events.rs:421,589` take `i64`; `polyoxide-gamma/src/types.rs:479` and `polyoxide-sports/src/update.rs:57` are `Option<u64>`; `types.rs:333` is `Option<String>`. Shipped in v0.38.0; S2 puts gamma and sports in one crate, a natural point to align them (a breaking change, so S2's rename stage).
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-publish-order-script-and-resumable-releases.md`
  summary: Restrict the `cargo` and `pypi` GitHub environments to deployments from `main` (AD-25's second guard against fork-triggered publishes); a repository setting the agent does not change.
  evidence: `gh api repos/dilettante-trading/polyoxide/environments/{cargo,pypi}` returns `deployment_branch_policy: null` (2026-10-08). The exact `gh api` commands are in the spec's Design Notes.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-publish-order-script-and-resumable-releases.md`
  summary: Before the next release from `main`, confirm the crates.io token (`CARGO_REGISTRY_TOKEN`) has the publish-new scope for `polyoxide-cli`, which the release loop now publishes for the first time.
  evidence: `polyoxide-cli` returns 404 on crates.io; it sorts last in the publish order, so a token without publish-new uploads every other crate and then fails, leaving the release untagged until a re-run with a fixed token. AD-25 and Story 4.11 also ask for the crate-scope check before the S1 release.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-3-package-every-crate-on-every-pr.md`
  summary: `main` has no branch protection or required status checks, so a red CI job (the new Package job included) does not block a merge; it then withholds the release on `main`. Make the CI jobs required checks (repository setting), and widen CLAUDE.md's "A red doc build costs more than it looks" paragraph to every CI job.
  evidence: `gh api repos/dilettante-trading/polyoxide/branches/main` reports `protected: false` (2026-10-08); `release.yml` proceeds only on a successful CI run. Required checks match by job name: the job is "Package (publish dry run)".
- source_spec: `_bmad-output/implementation-artifacts/spec-1-3-package-every-crate-on-every-pr.md`
  summary: Add the package gate's local commands (`python3 scripts/publish_order.py check-manifests`, `cargo publish --workspace --dry-run --no-verify --locked`) to CLAUDE.md's Build & Development Commands block, so an agent runs them before pushing.
  evidence: That block lists the other must-pass gates (clippy, doc, fmt) but not this one.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-3-package-every-crate-on-every-pr.md`
  summary: When Epic 5 adds `tombstones/*`, extend the CI package job to dry-run each tombstone with `--manifest-path`; the workspace dry run cannot see them because the root `Cargo.toml` excludes `tombstones`.
  evidence: `finish_release.sh` publishes tombstones after the workspace crates; no PR-time check packages them.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-4-1-5-registration-and-derived-nightly-rows.md`
  summary: With only part of a credential set configured as repository secrets, the unset ones arrive as `""` and `Account::from_env()` accepts empty L2 credentials, so clob's live tests fail as real instead of auth-gated. Make the credential loaders treat `""` as absent (AD-14), which Epic 2's shared loaders own.
  evidence: The nightly jobs now wire each target's declared secrets; GitHub expands an unset secret to an empty string. With every secret empty the tests panic auth-gated (verified); a partial set reaches `Account::from_env()` (`polyoxide-clob/src/account/mod.rs:185-208`), which does not filter empty values.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-6-1-8-gates-removal-gate-and-agent-guide.md`
  summary: Add `docs/MUTANTS.md` rows, each proven, for the rules the guide calls "rules that bite" beyond Story 1.8's five: the per-signer layer keeping `allow_burst` (`polyoxide-core/src/signer_limit.rs:264`), the reserved tenth (`RESERVED_FRACTION`, `rate_limit.rs:177-186`), and Binance's 418 hold (`polyoxide-binance/src/usdm/request.rs:111-113`).
  evidence: Story 1.8's acceptance criteria name five rules; these three are equally load-bearing and currently have no mutant on record.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-6-1-8-gates-removal-gate-and-agent-guide.md`
  summary: Unverified (would be medium): cargo-semver-checks 0.51.0 may not report removed cross-crate re-exports, which make up most of the umbrella `polyoxide` crate's API (`pub use polyoxide_clob;`, `prelude`), so the removal gate may miss them.
  evidence: The tool's own `snapshot_tests.rs:396-399` says cross-crate items are not yet supported; not confirmed by a run. Settle by removing one `pub use` from `polyoxide/src/lib.rs` in a scratch tree and running `api_removals.py check`; if unreported, add the umbrella's re-export paths to the compile-test list in `docs/s1-removals.md`.
- source_spec: `_bmad-output/implementation-artifacts/spec-2-1-2-2-classification-vocabulary-and-today-s-enums.md`
  summary: In Story 4.3's shared socket table (`impl_ws_classification!`), read `Retry-After` from a refused WebSocket upgrade (429, 418) into `Class::RateLimited`/`Restricted`, and classify clob's "no addresses resolved" DNS miss (built as `WebSocketError::InvalidMessage`, `polyoxide-clob/src/ws/client.rs:104`) as `Network` rather than `InvalidRequest`.
  evidence: Today the five socket `Classify` impls drop the handshake's `Retry-After`, and the zero-address DNS case is a transient failure classed non-retriable; fixing either in Epic 2 would change a constructed variant's `Display` (AD-16) or duplicate the header read five times before Story 4.3 makes it one table.
- source_spec: `_bmad-output/implementation-artifacts/spec-2-1-2-2-classification-vocabulary-and-today-s-enums.md`
  summary: Make a 451 region block a non-fault (`is_fault() == false`) on the socket side too: `RtdsError::Server { status: 451 }` and the shared handshake-status rule (`class_for_handshake_status`), as the HTTP status-rule impls already do since 543d1cb. Fold it into Story 4.3's single socket table.
  evidence: Bundle C's review made 451 a non-fault for `ApiError::Api`, perps `VenueError`, data `V2Error` and `BinanceError::Venue`; the socket paths were outside that patch's list, so a 451 there still counts as a fault.
