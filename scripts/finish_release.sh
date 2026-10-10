#!/usr/bin/env bash
# Publish every crate version crates.io does not have yet, in dependency order.
#
# release.yml's publish job runs this. Each attempt asks scripts/publish_order.py
# for the pairs still missing, so a re-run publishes only what the last run did
# not, and nothing is uploaded twice.
#
# To recover a release, run the Release workflow on main instead: this script
# publishes to crates.io only, with no tag, GitHub release or PyPI wheels. Run by
# hand, it refuses a dirty tree, a HEAD that is not on origin/main, or one CI has
# not passed on, since every publish is --no-verify. It takes
# CARGO_REGISTRY_TOKEN from the environment or a prior `cargo login`.
#
# Exits 0 when crates.io has every version, 3 when the release would register
# too many new crate names, 4 when the workspace cannot be published as it
# stands (publish_order.py says why), and 1 otherwise.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$ROOT"

ATTEMPTS=3

if [ -z "${GITHUB_ACTIONS:-}" ]; then
  if [ -n "$(git status --porcelain)" ]; then
    echo "error: the working tree has uncommitted changes; publish only a commit on main." >&2
    exit 1
  fi
  git fetch --quiet origin main
  if ! git merge-base --is-ancestor HEAD origin/main; then
    echo "error: HEAD is not on origin/main; publish only a commit on main." >&2
    exit 1
  fi
  if ! python3 scripts/publish_order.py ci-passed "$(git rev-parse HEAD)"; then
    echo "error: CI has not passed on HEAD, and every publish is --no-verify." >&2
    exit 1
  fi
fi

# Publish the `name version manifest_path` lines on stdin. The workspace members
# go in one cargo invocation, which orders them and waits for each to reach the
# index before publishing what depends on it. The tombstones follow, one by one.
# The `${a[@]+...}` expansions keep an empty array legal under `set -u` on bash 3.2.
publish() {
  local name manifest workspace=() tombstones=()
  while read -r name _ manifest; do
    case "$manifest" in
      "$ROOT"/tombstones/*) tombstones+=("$manifest") ;;
      *) workspace+=(-p "$name") ;;
    esac
  done
  if [ "${#workspace[@]}" -gt 0 ]; then
    cargo publish --no-verify ${workspace[@]+"${workspace[@]}"} || return 1
  fi
  for manifest in ${tombstones[@]+"${tombstones[@]}"}; do
    cargo publish --no-verify --manifest-path "$manifest" || return 1
  done
}

for attempt in $(seq "$ATTEMPTS"); do
  status=0
  pairs=$(python3 scripts/publish_order.py list --max-new-names 5) || status=$?
  # Too many new names. Retrying cannot change that.
  if [ "$status" -eq 3 ]; then
    exit 3
  fi
  if [ "$status" -eq 4 ]; then
    echo "::error::The workspace cannot be published as it stands; this is not transient, so not retrying."
    exit 4
  fi
  if [ "$status" -eq 0 ]; then
    if [ -z "$pairs" ]; then
      echo "crates.io has every version; nothing left to publish."
      exit 0
    fi
    echo "Attempt $attempt of $ATTEMPTS, publishing:"
    echo "$pairs"
    if publish <<< "$pairs"; then
      continue
    fi
  fi
  echo "Attempt $attempt of $ATTEMPTS failed; waiting 30s."
  sleep 30
done

# The last attempt's publish is confirmed here. A recount that cannot reach
# crates.io says nothing about what is published, so it is retried, and a
# recount that never answers is reported apart from one that finds pairs missing.
for try in $(seq "$ATTEMPTS"); do
  status=0
  pairs=$(python3 scripts/publish_order.py list) || status=$?
  if [ "$status" -eq 4 ]; then
    echo "::error::The workspace cannot be published as it stands; this is not transient, so not retrying."
    exit 4
  fi
  if [ "$status" -eq 0 ]; then
    if [ -z "$pairs" ]; then
      echo "crates.io has every version; nothing left to publish."
      exit 0
    fi
    echo "::error::Still unpublished after $ATTEMPTS attempts:"
    echo "$pairs"
    exit 1
  fi
  if [ "$try" -lt "$ATTEMPTS" ]; then
    echo "Recount $try of $ATTEMPTS failed; waiting 30s."
    sleep 30
  fi
done
echo "::error::Could not confirm that crates.io has every version: the recount failed $ATTEMPTS times. Re-run to finish or confirm the release."
exit 1
