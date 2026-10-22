---
paths:
  - "scripts/{publish_order.py,finish_release.sh}"
  - ".github/workflows/release.yml"
  - ".github/scripts/tests/test_{publish_order,finish_release,release_workflow,changelog}.py"
  - "CHANGELOG.md"
  - "cliff.toml"
---

# Releasing

Anything that publishes (`scripts/finish_release.sh`, `cargo publish` without `--dry-run`,
`gh workflow run Release`) cannot be taken back: no old version is ever yanked, and every publish
is `--no-verify`. A version bump is its own commit on `main`.

## The order is computed

`scripts/publish_order.py` reads `cargo metadata` and puts each publishable member (`publish`
unset, or naming `crates-io`) after every member it needs on crates.io first: its normal and build
dependencies, and any dev-dependency that carries a version, since that one stays in the published
manifest. A path-only dev-dependency is stripped at publish time and orders nothing. A cycle, or a
normal or build dependency on a `publish = false` member, fails the release before anything is
uploaded. Today's order is generated into CLAUDE.md. `polyoxide-py` is `publish = false`; its
wheels are built and published to PyPI by a separate step of the release workflow.

## Tombstones

S2 retired seven crates (`clob`, `data`, `gamma`, `perps`, `relay`, `rtds` and `sports`), and the
S2 release, 0.40.0, published each once more from `tombstones/<dir>/` as `polyoxide-<dir>`: a crate
with no dependencies whose crate doc, README and one `#[cfg(not(docsrs))] compile_error!` name
`polyoxide-polymarket`, the feature and the rename manifest, and which declares its crate's 0.39
features, empty, so a dependency that names one still reaches the error. The switch to S3 deleted
`tombstones/`. The machinery stays for a stage that retires a crate:

- the root manifest still excludes `tombstones`, or `cargo metadata` could not read them;
- `publish_order.py check-manifests` holds the directory to the crates `TOMBSTONES` lists for the
  workspace's stage (none for S3, so an absent directory passes);
- the release publishes any tombstone after every workspace crate;
- CI's package job packages each one it finds (`cargo package --no-verify --manifest-path`), which
  the workspace dry run cannot see.

## One resumable loop

`release.yml`'s publish job runs `scripts/finish_release.sh`, which makes up to three attempts.
Each asks `publish_order.py list --max-new-names 5` for the (crate, version) pairs crates.io lacks,
publishes the workspace pairs in one `cargo publish --no-verify -p A -p B …` (cargo orders them
and waits for each to reach the index), then each tombstone by `--manifest-path`. A final recount
confirms nothing is left, so a re-run after a partial release publishes only what is left.

- Exit 3: more than five new crate names. crates.io rate-limits new registrations, so split the
  release.
- Exit 4: the workspace cannot be published as it stands (a cycle, a dependency on a
  `publish = false` member, a manifest `cargo metadata` cannot read).
- Retrying cannot help either, so the loop stops at once on both. A recount that cannot reach
  crates.io ends in "could not confirm", which is not "still unpublished".

## What releases

`release.yml` runs for a push to `main` in this repository, or a manual run (`workflow_dispatch`)
on `main`. Its `branches: [main]` filter is not the guard, since it also matches a fork's pull
request from a branch named `main`; the `version` job checks that the CI run was a `push` to this
repository's `main`. It triggers on `workflow_run: [CI]` with `conclusion == 'success'`, so a red
CI job withholds the release. Then `publish_order.py decide` reads the `v<version>` tag on origin
(the peeled commit, since release tags are annotated), and the first matching rule wins:

1. A manual run on a commit CI has not passed fails, since every publish is `--no-verify`.
2. No tag: release. So the next green push to `main` releases any version still untagged, a
   fix-forward or an empty commit after a red version-bump commit included.
3. A tag at this commit and a GitHub release: skip. The run is a re-run of a finished release.
4. A tag at this commit and no GitHub release: release, resuming one whose last jobs failed.
   Publishing skips what crates.io has, and the tag is not pushed twice.
5. A tag at another commit, on a push that leaves the version as its parent had it: skip, with a
   plain log line. Every ordinary push lands here.
6. A tag at another commit, with the version lower than the parent's: skip with a warning.
7. A tag at another commit otherwise fails the run with `::error::`: the version was already
   released, so bump it.

A lookup that fails (`git ls-remote`, `gh`) fails the run rather than reading as "not released".

## The `semver` job

`release.yml`'s `semver` job runs cargo-semver-checks against the newest release tag behind the
commit, before anything publishes, and lets it infer the release type from the two versions. On a
0.x patch bump it fails on any major-level change, not only a removal: a new required field, a
changed signature, an enum made non-exhaustive. It also runs the compile test over the stage's
listing's paths (`docs/s3-removals.md`'s in S3; the manifest's in S2). When it fails, raise the
0.x minor in a new version-bump commit on `main`; the release that failed was never tagged, so the
next green push releases the new version.

## Recovering a release

Run Release by hand on `main` (`gh workflow run Release --ref main`): it publishes what is
missing and finishes the tag, the GitHub release and the wheels. Running
`scripts/finish_release.sh` by hand is a crates.io-only fallback: it refuses a dirty tree, a HEAD
that is not on `origin/main`, or one CI has not passed on (`publish_order.py ci-passed`), and it
takes `CARGO_REGISTRY_TOKEN` or a prior `cargo login`.

## The second guard

The `cargo` and `pypi` environments accept deployments only from `main` (a custom branch policy
naming `main` on each), so a run from any other branch, an edited `release.yml` on a feature
branch among them, cannot reach the tokens. It is a repository setting, not a file:
`gh api repos/dilettante-trading/polyoxide/environments/<name>/deployment-branch-policies` shows
it.
