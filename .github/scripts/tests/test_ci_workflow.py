"""Checks on .github/workflows/ci.yml's gates.

The package job checks manifests and can never publish. It runs on every pull
request, so it must hold no registry token, never drop `--dry-run`, and never be
skipped by a condition. The MSRV, per-feature, removal and test-body gates (AD-22)
run the commands and pins their stories fixed.
"""

from __future__ import annotations

import importlib.util
import re
import sys
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
    assert _job_runs("features")[0] == (
        "cargo hack check --workspace --each-feature --no-dev-deps --ignore-private")


def _load_publish_order():
    """`scripts/publish_order.py`, which lives outside this uv project."""
    if "publish_order" in sys.modules:
        return sys.modules["publish_order"]
    spec = importlib.util.spec_from_file_location("publish_order", REPO / "scripts" / "publish_order.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def http_modules(metadata: dict) -> list[str]:
    """The members that send on core's HTTP client: each declares a venue and
    depends directly on polyoxide-core. Each must pin its header set."""
    publish_order = _load_publish_order()
    return sorted(
        p["name"] for p in publish_order.members(metadata)
        if ((p.get("metadata") or {}).get("polyoxide") or {}).get("venue")
        and any(dep["name"] == "polyoxide-core" and dep["kind"] is None
                for dep in p["dependencies"]))


def test_the_features_job_runs_each_header_pin_in_its_minimal_build() -> None:
    """One `-p` per command, so no module's features are unified with another's;
    the workspace build runs the same files, against the same literal (AD-18).
    The modules come from `cargo metadata`, so a new venue crate on core's client
    fails here until it pins its headers and the step names it."""
    metadata = _load_publish_order().cargo_metadata(REPO / "Cargo.toml")
    modules = http_modules(metadata)
    assert len(modules) >= 6, f"too few HTTP modules found, so the check is vacuous: {modules}"
    [step] = [s for s in _steps("features") if "--test headers" in s.get("run", "")]
    named = []
    for command in step["run"].strip().splitlines():
        match = re.fullmatch(r"cargo test -p (\S+) --no-default-features --test headers", command)
        assert match, f"not one module's header pin with one `-p`: {command!r}"
        named.append(match[1])
    assert sorted(named) == modules
    for module in modules:
        assert (REPO / module / "tests" / "headers.rs").is_file(), module


def test_http_modules_are_venue_crates_on_core() -> None:
    def package(name: str, venue: str | None, deps: list[str]) -> dict:
        return {"id": name, "name": name,
                "metadata": {"polyoxide": {"venue": venue}} if venue else None,
                "dependencies": [{"name": d, "kind": None} for d in deps]}

    packages = [
        package("polyoxide-core", None, ["reqwest"]),
        package("polyoxide-kalshi", "kalshi", ["polyoxide-core"]),
        package("polyoxide-rtds", "polymarket", ["polyoxide-venue"]),
        package("polyoxide-cli", None, ["polyoxide-core"]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert http_modules(metadata) == ["polyoxide-kalshi"]


def test_the_s2_baseline_is_set_once_for_the_whole_workflow() -> None:
    """The removal gate and the test-body diff compare with the same tag."""
    assert CI["env"]["S2_BASELINE"] == "v0.39.0"
    for name, job in JOBS.items():
        assert "S2_BASELINE" not in (job.get("env") or {}), name
        assert "S1_BASELINE" not in yaml.safe_dump(job), name


def _checkout(job: str) -> dict:
    return next(s for s in _steps(job) if s.get("uses", "").startswith("actions/checkout"))


def test_the_removals_job_runs_the_s2_gate_against_the_s2_baseline() -> None:
    """Report-only through S2; Story 5.12 adds `--strict`."""
    assert _checkout("removals")["with"]["fetch-depth"] == 0
    assert _job_runs("removals") == [
        'python3 scripts/api_removals.py manifest --baseline "$S2_BASELINE" '
        '--expand target/rename-map.tsv']


def test_the_removals_job_uploads_the_regenerated_manifest_and_the_map_even_on_failure() -> None:
    steps = _steps("removals")
    [upload] = [s for s in steps if s.get("uses", "").startswith("actions/upload-artifact")]
    assert upload["if"] == "${{ always() }}"
    assert upload["with"]["path"].split() == ["target/api-removals/rename-manifest.md",
                                              "target/rename-map.tsv"]
    assert steps.index(upload) > next(i for i, s in enumerate(steps) if "run" in s)


def test_the_test_bodies_job_diffs_against_the_s2_baseline() -> None:
    assert _checkout("test-bodies")["with"]["fetch-depth"] == 0
    assert _job_runs("test-bodies") == [
        'python3 scripts/test_body_diff.py --baseline "$S2_BASELINE"']
    # Python only: nothing compiles, so no toolchain or wrapper is set up.
    assert JOBS["test-bodies"]["env"]["RUSTC_WRAPPER"] == ""
    assert not [u for u in _uses("test-bodies") if not u.startswith("actions/checkout")]


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


@pytest.mark.parametrize("job", ["msrv", "features", "removals", "test-bodies"])
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


def _caches(job: str) -> dict[str, dict]:
    return {s["with"]["path"]: s for s in _steps(job)
            if s.get("uses", "").startswith("actions/cache")}


def test_the_baseline_cache_is_keyed_on_the_baseline_and_both_pins() -> None:
    """A key that missed one of the three would restore a baseline build made by
    another toolchain, tool or tag."""
    cache = _caches("removals")["target/semver-checks/git-*"]
    toolchain = next(u for u in _uses("removals") if u.startswith("dtolnay/")).split("@")[1]
    tool = _installed("removals")[0].split("@")[1]
    key = cache["with"]["key"]
    assert "${{ env.S2_BASELINE }}" in key
    assert f"rust-{toolchain}" in key and f"cargo-semver-checks-{tool}" in key
    # Restored before the tool runs, or it rebuilds cold anyway.
    steps = _steps("removals")
    assert steps.index(cache) < next(i for i, s in enumerate(steps) if "run" in s)


def test_the_path_diffs_baseline_cache_is_keyed_on_the_baseline_and_the_toolchain() -> None:
    """The JSON's own stamp names the tree, `rustc -vV` and the rustdoc command, so a
    wrong entry is rebuilt rather than read; the key, which also names the script
    that builds and walks it, keeps one from being restored at all."""
    caches = _caches("removals")
    assert sorted(caches) == ["target/api-removals/baseline", "target/semver-checks/git-*"]
    cache = caches["target/api-removals/baseline"]
    toolchain = next(u for u in _uses("removals") if u.startswith("dtolnay/")).split("@")[1]
    key = cache["with"]["key"]
    assert "${{ env.S2_BASELINE }}" in key and f"rust-{toolchain}" in key
    assert "${{ hashFiles('scripts/public_api.py') }}" in key
    steps = _steps("removals")
    assert steps.index(cache) < next(i for i, s in enumerate(steps) if "run" in s)
