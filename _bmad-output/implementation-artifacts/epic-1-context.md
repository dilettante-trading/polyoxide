# Epic 1 Context: Reliable releases and one-place registration

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

This is the first epic of stage S1, the internals release in which crate names do not change, and it merges before any other S1 work. Hand-kept registration lists have drifted. `polyoxide-cli` has never reached crates.io, `release.yml` and `finish_release.sh` disagree on what they publish, and a fork PR from a branch named `main` can trigger a release. After this epic:
- releases publish every publishable crate, can be resumed after a failure, and fail loudly on a reused version;
- each crate is registered once, in its own `Cargo.toml` metadata, and the docs tables, CLAUDE.md lists and nightly and schema rows are generated from it;
- the CI gates that later S1 epics rely on are in place;
- the agent guide and the mutant ledger are in the repository.

## Stories

- Story 1.1: Re-baseline on current main
- Story 1.2: Publish-order script and resumable releases
- Story 1.3: Package every crate on every PR
- Story 1.4: Registration metadata and the generator
- Story 1.5: Nightly and schema rows derived from metadata
- Story 1.6: Workspace hygiene gates
- Story 1.7: The S1 removal gate
- Story 1.8: The agent guide lands

## Requirements & Constraints

- **One registration source.**
  - Adding a crate edits only its own metadata and files. Everything else is derived from that metadata or checked against it in CI.
  - Generated output must reproduce today's hand-written content, nightly rows included, apart from deliberate corrections.
  - `spec:<id>` labels do not change.
- **Releases.**
  - A release publishes only the crate versions missing from crates.io, counting only members whose `publish` is not `false`.
  - It fails when it would add more than five new crate names.
  - It fails when the version's tag already exists at another SHA.
  - It fails on a removal unless the bump raises the 0.x minor.
  - It runs only for a push to `main` in this repository. If `main` fixes this guard first, verify that fix rather than rewrite it.
- **Versions.** Never bump the version on a loom or integration branch. The bump is a separate, final commit on `main`, made after `git fetch` and a crates.io check. Read versions from `cargo metadata`, never by regex.
- **CI gates.** New gates are jobs in `ci.yml`, so a red one withholds the release. Each must pass on the current tree. The existing gates still hold: fmt, and clippy and rustdoc with `-D warnings`.
- **Removals.** Every public item that S1 removes goes on a deliberate list. Any other removal fails CI. Removed paths get no shims or re-exports.
- **Standing rules.** When this epic supersedes a CLAUDE.md rule (the hand-written publish order, crate graph and nightly lists), edit the rule in the same change. Text inside a generated region changes only through the generator.

## Technical Decisions

- **Metadata.** Each crate has a `[package.metadata.polyoxide]` table keyed per test target. It holds venue and product ids, the README line, covered mirrors, `gate_exceptions`, `identifiers`, and `live.<target> = { suite, timeout, features, secrets }`, where `secrets` lists exact env names. Mirrors that belong to no crate (bridge, combos-rfq), and the exclusions, go in `[workspace.metadata.polyoxide.mirrors]`. The schema is fixed from S1 on; S2 only moves entries.
- **`scripts/gen_registry.py`.**
  - It is the only writer between the `generated:begin <id>` and `generated:end <id>` markers. Each marker uses its host file's comment leader, and the generator preserves indentation.
  - It generates one nightly job per (crate, suite) instead of a matrix, because secrets cannot be referenced from a matrix or `if:`.
- **`scripts/publish_order.py`.**
  - It orders crates topologically from `cargo metadata`, ignoring path-only dev-dependencies.
  - It asks the crates.io API which versions are missing, sending the User-Agent that crates.io requires.
  - It lists `tombstones/*` after the workspace crates, for S2.
  - `release.yml` and `finish_release.sh` share one resumable loop that publishes only what the script lists.
- **Removal gate.**
  - It runs `cargo semver-checks --baseline-rev <S1 start tag> --release-type patch`, so removals are reported across a 0.x minor bump.
  - cargo-semver-checks 0.51.0 is pinned together with its toolchain.
  - `scripts/api_removals.py` checks each reported removal against `docs/s1-removals.md`, which starts empty and stays cumulative until the S1 release.
  - The gate is report-only during S2 and fails on any removal from S3 on.
- **Workspace.**
  - The workspace sets `resolver = "3"`.
  - `rustls` is declared once in the workspace, with `ring` and `std`. This is DRIFT R5, because clob lacks `std` today, and it goes in its own commit naming R5.
  - The MSRV job runs 1.91 `check` and `doc` without `-D warnings`.
  - The per-feature job uses cargo-hack 0.6.45.
- **Docs.**
  - `docs/ARCHITECTURE.md` is only ever regenerated from the architecture spine, and the spine wins until it is.
  - `docs/MUTANTS.md` lists each mutation-tested rule, never in the guide, and each listed mutant must actually make its test fail.

## Cross-Story Dependencies

- **1.1 goes first.** It records the S1 start tag (v0.38.1 expected), which 1.7 uses as its baseline.
- **1.2 merges before Epics 2–4 create any new crate.** No release is cut from `main` until it lands. After it, the generator (1.4) is next in the S1 order.
- **1.3 depends on 1.2,** which corrects `polyoxide-cli`'s manifest, so that 1.3's packaging job can pass on it. The job skips `publish = false` members.
- **1.5 builds on 1.4.** Epic 2's credential loaders consume the `secrets` declarations.
- **1.7 is a declared predecessor of Epics 2–4.** No story that removes a public item lands before it.
- **Later epics use this tooling.** Story 4.11 cuts the S1 release with it, and Epic 5 moves metadata entries and publishes tombstones through the same loop.
