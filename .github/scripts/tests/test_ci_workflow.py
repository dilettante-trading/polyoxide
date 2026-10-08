"""Checks on .github/workflows/ci.yml's gates.

The package job checks manifests and can never publish. It runs on every pull
request, so it must hold no registry token, never drop `--dry-run`, and never be
skipped by a condition. The MSRV, per-feature and removal gates (AD-22) run the
commands and pins their stories fixed.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[3]
CI = yaml.safe_load((REPO / ".github" / "workflows" / "ci.yml").read_text())
PACKAGE = CI["jobs"]["package"]


def _runs() -> list[str]:
    return [step["run"] for step in PACKAGE["steps"] if "run" in step]


def test_the_package_job_only_dry_runs() -> None:
    publishes = [run for run in _runs() if "cargo publish" in run]
    assert publishes == ["cargo publish --workspace --dry-run --no-verify --locked"]


def test_the_package_job_checks_manifest_metadata() -> None:
    assert "python3 scripts/publish_order.py check-manifests" in _runs()


def test_the_package_job_holds_no_secret() -> None:
    text = yaml.safe_dump(PACKAGE)
    assert "secrets." not in text
    assert "CARGO_REGISTRY_TOKEN" not in text


def test_the_package_job_always_runs() -> None:
    assert "if" not in PACKAGE
    assert "needs" not in PACKAGE


# --- the gates of AD-22 ------------------------------------------------------

JOBS = CI["jobs"]


def _steps(job: str) -> list[dict]:
    return JOBS[job]["steps"]


def _uses(job: str) -> list[str]:
    return [step["uses"] for step in _steps(job) if "uses" in step]


def _job_runs(job: str) -> list[str]:
    return [step["run"] for step in _steps(job) if "run" in step]


def _installed(job: str) -> list[str]:
    return [step["with"]["tool"] for step in _steps(job)
            if step.get("uses", "").startswith("taiki-e/install-action")]


def test_the_msrv_job_builds_on_the_declared_rust_version() -> None:
    import tomllib

    msrv = tomllib.loads((REPO / "Cargo.toml").read_text())["workspace"]["package"]["rust-version"]
    assert f"dtolnay/rust-toolchain@{msrv}" in _uses("msrv")
    assert _job_runs("msrv") == ["cargo check --workspace --all-features",
                                 "cargo doc --no-deps --workspace --all-features"]


def test_the_msrv_job_does_not_deny_warnings() -> None:
    """Lints change between releases; the deny-warnings doc gate stays on stable."""
    text = yaml.safe_dump(JOBS["msrv"])
    assert "-D warnings" not in text and "RUSTDOCFLAGS" not in text


def test_the_features_job_checks_each_feature_with_the_pinned_cargo_hack() -> None:
    assert _installed("features") == ["cargo-hack@0.6.45"]
    assert _job_runs("features") == [
        "cargo hack check --workspace --each-feature --no-dev-deps --ignore-private"]


def test_the_removals_job_runs_the_gate_against_the_s1_baseline() -> None:
    job = JOBS["removals"]
    assert job["env"]["S1_BASELINE"] == "v0.38.1"
    checkout = next(s for s in job["steps"] if s.get("uses", "").startswith("actions/checkout"))
    assert checkout["with"]["fetch-depth"] == 0
    assert _job_runs("removals") == [
        'python3 scripts/api_removals.py check --baseline "$S1_BASELINE"']


def test_the_semver_tool_and_its_toolchain_are_pinned_alike_in_both_workflows() -> None:
    """cargo-semver-checks reads only the rustdoc JSON versions some toolchains emit,
    so the pair moves together (AD-22), in CI and in the release."""
    release = yaml.safe_load((REPO / ".github" / "workflows" / "release.yml").read_text())
    semver = release["jobs"]["semver"]["steps"]
    pinned = {
        "ci": ([u for u in _uses("removals") if u.startswith("dtolnay/")], _installed("removals")),
        "release": ([s["uses"] for s in semver if s.get("uses", "").startswith("dtolnay/")],
                    [s["with"]["tool"] for s in semver
                     if s.get("uses", "").startswith("taiki-e/install-action")]),
    }
    assert pinned["ci"] == pinned["release"] == (
        ["dtolnay/rust-toolchain@1.99.0"], ["cargo-semver-checks@0.51.0"])


@pytest.mark.parametrize("job", ["msrv", "features", "removals"])
def test_each_new_gate_runs_on_every_pull_request(job: str) -> None:
    """A gate behind a condition or another job can be skipped, and a skipped job
    does not withhold the release."""
    assert "if" not in JOBS[job]
    assert "needs" not in JOBS[job]
    assert "secrets." not in yaml.safe_dump(JOBS[job])


@pytest.mark.parametrize("job", ["msrv", "features", "removals"])
def test_each_compiling_gate_installs_the_sccache_it_is_wrapped_in(job: str) -> None:
    """The workflow sets RUSTC_WRAPPER to sccache for every job."""
    assert CI["env"]["RUSTC_WRAPPER"] == "sccache"
    assert any(u.startswith("mozilla-actions/sccache-action") for u in _uses(job))


def test_the_baseline_cache_is_keyed_on_the_baseline_and_both_pins() -> None:
    """A key that missed one of the three would restore a baseline build made by
    another toolchain, tool or tag."""
    [cache] = [s for s in _steps("removals") if s.get("uses", "").startswith("actions/cache")]
    toolchain = next(u for u in _uses("removals") if u.startswith("dtolnay/")).split("@")[1]
    tool = _installed("removals")[0].split("@")[1]
    key = cache["with"]["key"]
    assert "${{ env.S1_BASELINE }}" in key
    assert f"rust-{toolchain}" in key and f"cargo-semver-checks-{tool}" in key
    assert cache["with"]["path"] == "target/semver-checks/git-*"
    # Restored before the tool runs, or it rebuilds cold anyway.
    steps = _steps("removals")
    assert steps.index(cache) < next(i for i, s in enumerate(steps) if "run" in s)
