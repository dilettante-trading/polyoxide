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


TOMBSTONE_STEP = "cargo package --no-verify --manifest-path"


def _tombstone_step() -> dict:
    [step] = [s for s in PACKAGE["steps"] if TOMBSTONE_STEP in s.get("run", "")]
    return step


def test_the_package_job_packages_each_tombstone_after_the_workspace() -> None:
    """The tombstones sit outside the workspace, so the workspace dry run never sees
    them (DW L22). Each is packaged by its own manifest, with `--no-verify`, since a
    tombstone's `compile_error!` fails any build, and whether or not the steps
    before it failed."""
    steps = PACKAGE["steps"]
    step = _tombstone_step()
    assert step["if"] == "${{ !cancelled() }}"
    workspace = next(i for i, s in enumerate(steps) if "cargo publish" in s.get("run", ""))
    assert steps.index(step) > workspace


def _run_tombstone_step(root: Path, tmp_path: Path) -> list[str]:
    """The step's script, run in `root` with a `cargo` that records its arguments."""
    import os
    import shutil
    import subprocess

    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    calls = tmp_path / "calls"
    cargo = bin_dir / "cargo"
    cargo.write_text(f'#!/bin/sh\necho "$*" >> "{calls}"\n')
    cargo.chmod(0o755)
    env = {**os.environ, "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}"}
    # GitHub runs a `run:` script with `bash -e`.
    subprocess.run([shutil.which("bash"), "-e", "-c", _tombstone_step()["run"]], cwd=root,
                   env=env, check=True)
    return calls.read_text().splitlines() if calls.exists() else []


def test_the_tombstone_step_packages_every_real_tombstone(tmp_path: Path) -> None:
    found = sorted(str(m.relative_to(REPO)) for m in REPO.glob("tombstones/*/Cargo.toml"))
    assert _run_tombstone_step(REPO, tmp_path) == [
        f"package --no-verify --manifest-path {manifest}" for manifest in found]


def test_the_tombstone_step_packages_nothing_once_they_are_gone(tmp_path: Path) -> None:
    """The commit after the release deletes tombstones/, and the step must pass on
    the empty glob without an edit of its own."""
    tree = tmp_path / "tree"
    tree.mkdir()
    assert _run_tombstone_step(tree, tmp_path) == []


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


def http_modules(metadata: dict) -> list[tuple[str, str]]:
    """The modules that send on core's HTTP client, as `(crate, product)` pairs: each
    member that declares a venue and depends directly on polyoxide-core holds one per
    product it declares, so `polyoxide-polymarket`'s `gamma` and `data` are two, but
    for a product behind a feature of its name that does not turn an optional
    polyoxide-core on, such as `rtds`, a socket alone. Each must pin its header set. A
    venue's own crate that declares no products holds no client to pin."""
    publish_order = _load_publish_order()

    def registration(p: dict) -> dict:
        return (p.get("metadata") or {}).get("polyoxide") or {}

    def on_core(p: dict, product: str) -> bool:
        features = p.get("features") or {}
        if product not in features or any(
                dep["name"] == "polyoxide-core" and dep["kind"] is None
                and not dep.get("optional") for dep in p["dependencies"]):
            return True
        seen, todo = set(), [product]
        while todo:
            feature = todo.pop()
            if feature in seen:
                continue
            seen.add(feature)
            for item in features.get(feature, ()):
                if item in ("dep:polyoxide-core", "polyoxide-core") or item.startswith(
                        "polyoxide-core/"):
                    return True
                if ":" not in item and "/" not in item:
                    todo.append(item)
        return False

    return sorted(
        (p["name"], product) for p in publish_order.members(metadata)
        if registration(p).get("venue")
        and any(dep["name"] == "polyoxide-core" and dep["kind"] is None
                for dep in p["dependencies"])
        for product in registration(p).get("products") or ()
        if on_core(p, product))


def header_pin(package: dict, product: str) -> tuple[str, str]:
    """A module's header pin in its minimal build, and the test file it runs. A crate
    with a feature named for the product holds it as a module behind that feature,
    whose suites carry its name (`headers_<product>`); any other is the module."""
    crate = package["name"]
    if product in (package.get("features") or {}):
        return (f"cargo test -p {crate} --no-default-features --features {product} "
                f"--test headers_{product}", f"tests/headers_{product}.rs")
    return f"cargo test -p {crate} --no-default-features --test headers", "tests/headers.rs"


# The umbrella crate, whose `full` turns on every module of every venue.
UMBRELLA = "polyoxide"
MINIMAL_PINS = "Header pins, minimal builds"
FULL_PINS = "Header pins, umbrella full build"


def full_pin(package: dict, product: str) -> str:
    """A module's header pin with the umbrella's `full` turned on beside its crate, so
    every feature the umbrella's modules switch on is unified into the module's build."""
    crate = package["name"]
    test = f"headers_{product}" if product in (package.get("features") or {}) else "headers"
    return f"cargo test -p {UMBRELLA} -p {crate} --features {UMBRELLA}/full --test {test}"


def umbrella_enables(features: dict, feature: str) -> set[str]:
    """The `<crate>/<feature>` items the umbrella's `feature` turns on, through the
    umbrella features it turns on, `?` dropped."""
    found, seen, todo = set(), set(), [feature]
    while todo:
        name = todo.pop()
        if name in seen:
            continue
        seen.add(name)
        for item in features.get(name, ()):
            if "/" in item:
                found.add(item.replace("?/", "/"))
            elif ":" not in item:
                todo.append(item)
    return found


def umbrella_missing(umbrella: dict, modules: list[tuple[str, str]], enabled: set[str],
                     unpublished: frozenset[str] = frozenset()) -> list[str]:
    """The `<crate>/<product>` of each module whose feature the umbrella's `full` (`enabled`)
    does not turn on. Only a crate that is `unpublished` (`publish = []` in cargo metadata) and
    not built by the umbrella is exempt: a published umbrella cannot name an unpublished crate,
    so such a crate's pin runs under `full` with no umbrella feature to turn on. A published
    venue crate the umbrella does not depend on is reported, since the umbrella forgot it."""
    built = {dep["name"] for dep in umbrella["dependencies"] if dep["kind"] is None}
    return [f"{crate}/{product}" for crate, product in modules
            if (crate in built or crate not in unpublished)
            and f"{crate}/{product}" not in enabled]


def test_the_features_job_runs_each_header_pin_in_its_minimal_build() -> None:
    """One `-p` per command, so no module's features are unified with another's;
    the workspace build runs the same files, against the same literal (AD-18).
    The modules come from `cargo metadata`, so a new venue module on core's client
    fails here until it pins its headers and the step names it."""
    metadata = _load_publish_order().cargo_metadata(REPO / "Cargo.toml")
    modules = http_modules(metadata)
    assert len(modules) >= 6, f"too few HTTP modules found, so the check is vacuous: {modules}"
    assert len({crate for crate, _ in modules}) < len(modules), (
        f"no crate holds two modules, so the module form is untested: {modules}")
    packages = {p["name"]: p for p in metadata["packages"]}
    [step] = [s for s in _steps("features") if s.get("name") == MINIMAL_PINS]
    commands = step["run"].strip().splitlines()
    for command in commands:
        assert re.fullmatch(r"cargo test -p \S+ --no-default-features"
                            r"(?: --features \S+)? --test headers\S*", command), (
            f"not one module's header pin with one `-p`: {command!r}")
    pins = [header_pin(packages[crate], product) for crate, product in modules]
    assert sorted(commands) == sorted(command for command, _ in pins)
    for (crate, _), (_, path) in zip(modules, pins):
        assert (REPO / crate / path).is_file(), f"{crate}/{path}"


def test_the_features_job_runs_each_header_pin_under_the_umbrellas_full() -> None:
    """The third build of each pin (AD-18): the umbrella's `full` with the module's
    crate, so a header one venue's module adds through the umbrella fails another's
    literal. The modules are the minimal step's, and `full` must turn on each one of a crate
    the umbrella depends on, or its pin would run without the module the build is meant to
    unify. A venue crate outside the umbrella (`publish = false`) is pinned under `full` too."""
    metadata = _load_publish_order().cargo_metadata(REPO / "Cargo.toml")
    modules = http_modules(metadata)
    assert len(modules) >= 6, f"too few HTTP modules found, so the check is vacuous: {modules}"
    assert len({crate for crate, _ in modules}) >= 2, (
        f"one venue crate alone, so the cross-venue build is untested: {modules}")
    packages = {p["name"]: p for p in metadata["packages"]}
    [step] = [s for s in _steps("features") if s.get("name") == FULL_PINS]
    commands = step["run"].strip().splitlines()
    for command in commands:
        assert re.fullmatch(rf"cargo test -p {UMBRELLA} -p \S+ --features {UMBRELLA}/full "
                            r"--test headers\S*", command), (
            f"not one module's header pin under the umbrella's full: {command!r}")
    assert sorted(commands) == sorted(full_pin(packages[crate], product)
                                      for crate, product in modules)
    enabled = umbrella_enables(packages[UMBRELLA]["features"], "full")
    unpublished = frozenset(name for name, p in packages.items() if p.get("publish") == [])
    missing = umbrella_missing(packages[UMBRELLA], modules, enabled, unpublished)
    assert not missing, f"the umbrella's full does not turn on {missing}"
    steps = [s.get("name") for s in _steps("features")]
    assert steps.index(FULL_PINS) == steps.index(MINIMAL_PINS) + 1, steps


def test_a_full_pin_names_the_umbrella_beside_the_module_crate() -> None:
    venue = {"name": "polyoxide-polymarket", "features": {"gamma": [], "data": []}}
    assert full_pin(venue, "gamma") == (
        "cargo test -p polyoxide -p polyoxide-polymarket --features polyoxide/full "
        "--test headers_gamma")
    crate = {"name": "polyoxide-bybit", "features": {"ws": []}}
    assert full_pin(crate, "linear") == (
        "cargo test -p polyoxide -p polyoxide-bybit --features polyoxide/full --test headers")


def test_umbrella_enables_follows_its_own_features() -> None:
    features = {"full": ["a-ws", "keychain"], "a-ws": ["a", "polyoxide-a/ws"],
                "a": ["dep:polyoxide-a", "polyoxide-a/a"], "keychain": ["polyoxide-a?/keychain"],
                "b": ["polyoxide-b/b"]}
    assert umbrella_enables(features, "full") == {
        "polyoxide-a/ws", "polyoxide-a/a", "polyoxide-a/keychain"}
    assert umbrella_enables(features, "b") == {"polyoxide-b/b"}


def test_a_venue_crate_outside_the_umbrella_needs_no_umbrella_feature() -> None:
    """The umbrella is published, so it cannot depend on an unpublished venue crate. Such a
    crate's pin still runs under `full`, but `full` has no feature of the crate's to turn on."""
    umbrella = {"dependencies": [
        {"name": "polyoxide-polymarket", "kind": None},
        # A dev-dependency is not built into `full`.
        {"name": "polyoxide-kalshi", "kind": "dev"}]}
    modules = [("polyoxide-kalshi", "predictions"), ("polyoxide-polymarket", "gamma")]
    unpublished = frozenset({"polyoxide-kalshi"})
    # Outside the umbrella and unpublished: nothing to turn on.
    on = {"polyoxide-polymarket/gamma"}
    assert umbrella_missing(umbrella, modules, on, unpublished) == []
    # Inside it, the module's feature is still required.
    assert umbrella_missing(umbrella, modules, set(), unpublished) == [
        "polyoxide-polymarket/gamma"]
    # A crate the umbrella builds is checked whatever its name (the published one, which this
    # umbrella does not build, is reported too).
    built = {"dependencies": [{"name": "polyoxide-kalshi", "kind": None}]}
    assert umbrella_missing(built, modules, set(), unpublished) == [
        "polyoxide-kalshi/predictions", "polyoxide-polymarket/gamma"]
    # A published crate outside the umbrella is still reported: the umbrella forgot it.
    assert umbrella_missing(umbrella, modules, {"polyoxide-polymarket/gamma"}) == [
        "polyoxide-kalshi/predictions"]


def test_the_features_job_checks_each_module_with_specta() -> None:
    """A module's `specta` extras are weak features (`specta?/chrono`), which neither
    `--each-feature` (`specta` alone) nor `--all-features` (every extra at once)
    builds apart, so each module of a crate with `specta` is checked with it."""
    metadata = _load_publish_order().cargo_metadata(REPO / "Cargo.toml")
    packages = {p["name"]: p for p in metadata["packages"]}
    expected = sorted(
        f"cargo check -p {crate} --no-default-features --features {product},specta"
        for crate, product in http_modules(metadata)
        if {product, "specta"} <= set(packages[crate].get("features") or {}))
    assert len(expected) >= 2, f"too few modules with specta, so the check is vacuous: {expected}"
    [step] = [s for s in _steps("features") if ",specta" in s.get("run", "")]
    assert sorted(step["run"].strip().splitlines()) == expected


def test_the_features_job_runs_clobs_refusal_without_gamma() -> None:
    """`clob` does not imply `gamma`, and the arm that refuses a proxy order with no
    funder compiles only without it, which no other build runs."""
    [step] = [s for s in _steps("features") if s.get("name") == "clob without gamma"]
    assert step["run"].strip() == (
        "cargo test -p polyoxide-polymarket --no-default-features --features clob "
        "--test mock_clob -- a_proxy_order_with_no_funder_is_refused_without_gamma --exact")
    test = REPO / "polyoxide-polymarket" / "tests" / "mock_clob.rs"
    assert "async fn a_proxy_order_with_no_funder_is_refused_without_gamma(" in test.read_text()


def test_the_features_job_runs_trading_clob_without_gamma() -> None:
    """`trading_clob` compiles three refusals of a proxy client only without `gamma`,
    which no other build runs, so the step must exist and the file must still hold one."""
    [step] = [s for s in _steps("features") if s.get("name") == "clob trading without gamma"]
    assert step["run"].strip() == (
        "cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data "
        "--test trading_clob")
    test = REPO / "polyoxide-polymarket" / "tests" / "trading_clob.rs"
    assert '#[cfg(not(feature = "gamma"))]' in test.read_text()


def test_http_modules_are_venue_crates_on_core() -> None:
    def package(name: str, venue: str | None, deps: list[str], products=("x",),
                features=None, optional=()) -> dict:
        return {"id": name, "name": name, "features": features or {},
                "metadata": ({"polyoxide": {"venue": venue, "products": list(products)}}
                             if venue else None),
                "dependencies": [{"name": d, "kind": None, "optional": d in optional}
                                 for d in deps]}

    packages = [
        package("polyoxide-core", None, ["reqwest"]),
        package("polyoxide-kalshi", "kalshi", ["polyoxide-core"]),
        package("polyoxide-socket", "polymarket", ["polyoxide-venue"]),
        package("polyoxide-cli", None, ["polyoxide-core"]),
        # A module is on core when its feature turns core on, directly or through
        # another feature; `rtds` and `sports` are sockets alone.
        package("polyoxide-polymarket", "polymarket", ["polyoxide-core"],
                products=("gamma", "data", "perps", "rtds", "sports"),
                features={"gamma": ["dep:polyoxide-core"], "shared": ["dep:polyoxide-core"],
                          "data": ["shared"], "perps": ["polyoxide-core/x"],
                          "rtds": ["dep:polyoxide-ws", "tokio/time"],
                          "sports": ["dep:polyoxide-ws", "polyoxide-core?/keychain"]},
                optional=("polyoxide-core",)),
        # A crate that always builds core holds every product on it, a product behind
        # a feature of its name included.
        package("polyoxide-bybit", "bybit", ["polyoxide-core"], products=("linear",),
                features={"linear": ["dep:polyoxide-ws"]}),
        package("polyoxide-hyperliquid", "hyperliquid", ["polyoxide-core"], products=()),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert http_modules(metadata) == [("polyoxide-bybit", "linear"),
                                      ("polyoxide-kalshi", "x"),
                                      ("polyoxide-polymarket", "data"),
                                      ("polyoxide-polymarket", "gamma"),
                                      ("polyoxide-polymarket", "perps")]


def test_docs_rs_builds_every_venue_feature_but_test_server() -> None:
    """docs.rs documents every module, `-ws` feature and extra of each venue crate, and
    leaves out `test-server`, which exposes captured frames for tests alone; a feature
    added to a crate and not to its list would go undocumented."""
    publish_order = _load_publish_order()
    metadata = publish_order.cargo_metadata(REPO / "Cargo.toml")
    venues = [p for p in publish_order.members(metadata)
              if ((p.get("metadata") or {}).get("polyoxide") or {}).get("venue")]
    assert len(venues) >= 2, f"too few venue crates found, so the check is vacuous: {venues}"
    for package in venues:
        listed = package["metadata"]["docs"]["rs"]["features"]
        assert sorted(listed) == sorted(set(package["features"]) - {"default", "test-server"}), (
            package["name"])
        assert len(listed) == len(set(listed)), (package["name"], listed)


def test_a_module_behind_its_feature_pins_its_own_suite() -> None:
    venue = {"name": "polyoxide-polymarket", "features": {"gamma": [], "data": []}}
    assert header_pin(venue, "gamma") == (
        "cargo test -p polyoxide-polymarket --no-default-features --features gamma "
        "--test headers_gamma", "tests/headers_gamma.rs")
    crate = {"name": "polyoxide-bybit", "features": {"ws": []}}
    assert header_pin(crate, "linear") == (
        "cargo test -p polyoxide-bybit --no-default-features --test headers",
        "tests/headers.rs")


def test_the_scripts_job_fetches_the_tag_the_venue_gate_reads() -> None:
    """test_venue_gate.py runs the gate on v0.39.0's core, which a shallow checkout
    lacks."""
    assert _checkout("scripts")["with"]["fetch-depth"] == 0
    assert "uv run pytest tests/ -v" in _job_runs("scripts")


def test_the_s3_baseline_is_set_once_for_the_whole_workflow() -> None:
    """The removal gate and the test-body diff compare with the same tag."""
    assert CI["env"]["S3_BASELINE"] == "v0.40.0"
    assert "S2_BASELINE" not in CI["env"]
    for name, job in JOBS.items():
        assert "S3_BASELINE" not in (job.get("env") or {}), name
        dumped = yaml.safe_dump(job)
        assert "S1_BASELINE" not in dumped and "S2_BASELINE" not in dumped, name


def _checkout(job: str) -> dict:
    return next(s for s in _steps(job) if s.get("uses", "").startswith("actions/checkout"))


def test_the_removals_job_runs_the_s3_gate_against_the_s3_baseline() -> None:
    """From stage S3 the gate is `check`, which reads docs/s3-removals.md; the
    manifest run, the frozen map's diff and the manifest artifact went with S2."""
    assert _checkout("removals")["with"]["fetch-depth"] == 0
    assert _job_runs("removals") == [
        'python3 scripts/api_removals.py check --baseline "$S3_BASELINE"']
    assert not [u for u in _uses("removals") if u.startswith("actions/upload-artifact")]


def test_the_test_bodies_job_diffs_against_the_s3_baseline() -> None:
    assert _checkout("test-bodies")["with"]["fetch-depth"] == 0
    assert _job_runs("test-bodies") == [
        'python3 scripts/test_body_diff.py --baseline "$S3_BASELINE"']
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
    assert "${{ env.S3_BASELINE }}" in key
    assert f"rust-{toolchain}" in key and f"cargo-semver-checks-{tool}" in key
    # Restored before the tool runs, or it rebuilds cold anyway.
    steps = _steps("removals")
    assert steps.index(cache) < next(i for i, s in enumerate(steps) if "run" in s)


def test_the_removals_job_caches_only_the_semver_baseline() -> None:
    """`check` builds no path diff, so S2's rustdoc JSON cache went with the manifest
    run."""
    assert sorted(_caches("removals")) == ["target/semver-checks/git-*"]


def test_kalshi_is_published_and_the_umbrellas_full_turns_on_its_modules() -> None:
    """Kalshi is a published venue crate the umbrella builds, so nothing exempts it from `full`.
    `unpublished` is left empty: a `publish = false` that came back, a Kalshi feature `full`
    forgot, or an umbrella that stopped depending on the crate fails here."""
    metadata = _load_publish_order().cargo_metadata(REPO / "Cargo.toml")
    packages = {p["name"]: p for p in metadata["packages"]}
    kalshi, umbrella = packages["polyoxide-kalshi"], packages[UMBRELLA]
    assert kalshi.get("publish") != [], "polyoxide-kalshi is still `publish = false`"
    built = {dep["name"] for dep in umbrella["dependencies"] if dep["kind"] is None}
    assert "polyoxide-kalshi" in built, f"the umbrella does not depend on Kalshi: {sorted(built)}"
    modules = http_modules(metadata)
    assert ("polyoxide-kalshi", "predictions") in modules, modules
    enabled = umbrella_enables(umbrella["features"], "full")
    assert {"polyoxide-kalshi/predictions", "polyoxide-kalshi/predictions-ws"} <= enabled, enabled
    assert umbrella_missing(umbrella, modules, enabled) == []
