"""Every live test target is registered, ignored, built with its features, and
handed exactly the secrets it reads.

A `tests/live_*.rs` becomes a nightly job only through its crate's
`[package.metadata.polyoxide.live.<target>]` entry, which scripts/gen_registry.py
turns into a job in nightly-behavioral.yml. These tests hold the three in step:
the live targets `cargo metadata` reports, the metadata entries, and the jobs in
the workflow. They also hold each entry to its file. A test without `#[ignore]`
would run in CI against a live host, a target missing `required-features`
compiles to an empty binary, and a secret the job does not pass is a test that
can never authenticate.

The secrets check is a static scan of each file (`gen_registry.env_names`). It
reads the names passed to polyoxide-test-support's credential loaders, which must
be string literals. A live target reads the environment no other way: a
`std::env::var`, `dotenvy` or a library's `from_env()` would read names the scan
cannot see, so each is refused (`gen_registry.direct_env_reads`).
"""

from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[3]


def _load_gen_registry():
    """`scripts/gen_registry.py`, which lives outside this uv project."""
    if "gen_registry" in sys.modules:
        return sys.modules["gen_registry"]
    spec = importlib.util.spec_from_file_location(
        "gen_registry", REPO / "scripts" / "gen_registry.py"
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


gen_registry = _load_gen_registry()
METADATA = gen_registry.publish_order.workspace_metadata()
REGISTRY = gen_registry.load(REPO, METADATA)
WORKFLOW = yaml.safe_load((REPO / ".github/workflows/nightly-behavioral.yml").read_text())
LIVE_SUITE = "./.github/actions/live-suite"


def live_targets(metadata: dict) -> dict[tuple[str, str], dict]:
    """Each member's `live_*` test target, keyed by (crate, target)."""
    return {
        (package["name"], target["name"]): target
        for package in gen_registry.publish_order.members(metadata)
        for target in package["targets"]
        if target["kind"] == ["test"] and target["name"].startswith("live_")
    }


def registered(registry) -> dict[tuple[str, str], object]:
    return {(crate.name, live.target): live for crate in registry.crates for live in crate.live}


def workflow_jobs(workflow: dict) -> dict[str, dict]:
    """The jobs that run the live-suite action, by job id."""
    return {
        job_id: job for job_id, job in workflow["jobs"].items()
        if any(step.get("uses") == LIVE_SUITE for step in job.get("steps", []))
    }


def _with(job: dict) -> dict:
    return next(step["with"] for step in job["steps"] if step.get("uses") == LIVE_SUITE)


def flagged_targets(flags: str) -> tuple[list[str], list[str]]:
    """The `--features` and `--test` values in a job's flags."""
    tokens = flags.split()
    values = {"--features": [], "--test": []}
    for flag, value in zip(tokens, tokens[1:]):
        if flag in values:
            values[flag] += value.split(",")
    return values["--features"], values["--test"]


def _path(crate: str, target: str) -> Path:
    return Path(live_targets(METADATA)[(crate, target)]["src_path"])


def _source(crate: str, target: str) -> str:
    """The target's file and every `mod x;` it declares, such as `tests/common/mod.rs`."""
    return gen_registry.target_source(_path(crate, target))


def _entry(key: tuple[str, str]):
    live = registered(REGISTRY).get(key)
    assert live is not None, f"{key[0]} {key[1]} is not registered; see test_every_live_target_is_registered"
    return live


# --- registration ------------------------------------------------------------


def test_every_live_target_is_registered() -> None:
    targets, entries = set(live_targets(METADATA)), set(registered(REGISTRY))
    assert targets == entries, (
        f"live test targets without a [package.metadata.polyoxide.live.<target>] entry: "
        f"{sorted(targets - entries)}; entries naming no target: {sorted(entries - targets)}"
    )


def test_every_live_file_is_a_target() -> None:
    """A `[[test]]` with a custom `path` could hide a live file from the check above."""
    on_disk = {
        path.resolve()
        for package in gen_registry.publish_order.members(METADATA)
        for path in Path(package["manifest_path"]).parent.glob("tests/live_*.rs")
    }
    targets = {Path(t["src_path"]).resolve() for t in live_targets(METADATA).values()}
    assert on_disk == targets


def test_an_unregistered_live_target_is_caught() -> None:
    metadata = {**METADATA, "packages": [dict(p) for p in METADATA["packages"]]}
    gamma = next(p for p in metadata["packages"] if p["name"] == "polyoxide-gamma")
    gamma["targets"] = gamma["targets"] + [{"name": "live_x", "kind": ["test"],
                                            "src_path": "/ws/polyoxide-gamma/tests/live_x.rs"}]
    assert set(live_targets(metadata)) - set(registered(REGISTRY)) == {("polyoxide-gamma", "live_x")}


# --- the generated jobs ------------------------------------------------------


def test_the_generated_jobs_equal_the_registered_entries() -> None:
    jobs = workflow_jobs(WORKFLOW)
    assert list(jobs) == [job.id for job in REGISTRY.jobs()]
    selected = set()
    for job in REGISTRY.jobs():
        written = jobs[job.id]
        inputs = _with(written)
        assert written["name"] == f"Live tests ({job.crate}, {job.suite})"
        assert written["timeout-minutes"] == job.timeout
        assert (inputs["crate"], inputs["suite"], inputs["flags"]) == (job.crate, job.suite, job.flags)
        features, tests = flagged_targets(inputs["flags"])
        assert sorted(features) == list(job.features)
        selected |= {(job.crate, test) for test in tests}
    assert selected == set(registered(REGISTRY))


def test_each_job_carries_only_its_declared_secrets() -> None:
    jobs = workflow_jobs(WORKFLOW)
    for job in REGISTRY.jobs():
        env = jobs[job.id].get("env", {})
        assert env == {name: f"${{{{ secrets.{name} }}}}" for name in job.secrets}, job.id


def test_no_secret_reaches_a_job_through_the_workflow_env() -> None:
    assert "secrets." not in yaml.safe_dump(WORKFLOW.get("env", {}))


def test_the_aggregate_needs_every_live_job() -> None:
    needs = WORKFLOW["jobs"]["aggregate"]["needs"]
    assert needs == [job.id for job in REGISTRY.jobs()]
    others = [job_id for job_id in WORKFLOW["jobs"] if job_id != "aggregate"]
    assert needs == others


def test_the_close_guard_reads_every_needed_job() -> None:
    steps = WORKFLOW["jobs"]["aggregate"]["steps"]
    close = next(step for step in steps if "gh issue close" in step.get("run", ""))
    guard = close["env"]["ALL_SUCCEEDED"]
    for result in ("failure", "cancelled", "skipped"):
        assert f"!contains(needs.*.result, '{result}')" in guard
    assert '[ "$ALL_SUCCEEDED" = "true" ]' in close["run"]
    assert "needs.test" not in yaml.safe_dump(WORKFLOW)


# --- ignored -----------------------------------------------------------------

# `#[test]`, any `#[<path>::test]` (`tokio::test`, `test_log::test`), and `#[rstest]`.
TEST_ATTRIBUTE = re.compile(r"^#\[(?:(?:\w+::)*test|rstest)\b")
FUNCTION = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")


def unignored(source: str) -> list[str]:
    """The test functions in `source` with no `#[ignore]` among their attributes."""
    found, attributes, pending = [], [], ""
    for number, line in enumerate(source.splitlines(), start=1):
        stripped = line.strip()
        if pending or stripped.startswith("#["):
            pending += stripped
            if pending.count("[") <= pending.count("]"):
                attributes.append(pending)
                pending = ""
            continue
        if stripped.startswith("//") or not stripped:
            continue
        function = FUNCTION.match(stripped)
        if function and any(TEST_ATTRIBUTE.match(a) for a in attributes):
            if not any(a.startswith("#[ignore") for a in attributes):
                found.append(f"{function[1]} (line {number})")
        attributes = []
    return found


@pytest.mark.parametrize("key", sorted(live_targets(METADATA)), ids="/".join)
def test_every_live_test_is_ignored(key: tuple[str, str]) -> None:
    found = [f"{file.relative_to(REPO)}: {test}"
             for file in gen_registry.module_files(_path(*key)) for test in unignored(file.read_text())]
    assert not found, (
        f"{key[0]} {key[1]} has tests without #[ignore], which CI would run against "
        f"the live host: {found}. Ignore them, or move an offline test to another file."
    )


def test_an_unignored_live_test_is_caught() -> None:
    source = """
#[tokio::test]
#[ignore]
async fn ignored_after() {}

#[ignore = "hits the host"]
#[tokio::test(flavor = "multi_thread")]
async fn ignored_before() {}

/// Documented, and still a test.
#[test]
fn offline() {}

mod nested {
    #[tokio::test]
    async fn also_live() {}
}

#[cfg(test)]
fn helper() {}
"""
    assert unignored(source) == ["offline (line 12)", "also_live (line 16)"]


@pytest.mark.parametrize(
    "attribute", ["#[test]", "#[tokio::test]", "#[test_log::test]", "#[tokio::test(flavor = \"multi_thread\")]",
                  "#[rstest]", "#[a::b::test]"])
def test_every_test_attribute_needs_an_ignore(attribute: str) -> None:
    assert unignored(f"{attribute}\nfn live() {{}}\n") == ["live (line 2)"]
    assert unignored(f"{attribute}\n#[ignore]\nfn live() {{}}\n") == []


@pytest.mark.parametrize("attribute", ["#[cfg(test)]", "#[test_case(1)]", "#[testing]", "#[derive(Debug)]"])
def test_other_attributes_are_not_tests(attribute: str) -> None:
    assert unignored(f"{attribute}\nfn helper() {{}}\n") == []


def test_a_multi_line_attribute_is_read_whole() -> None:
    source = "#[tokio::test(\n    flavor = \"multi_thread\",\n)]\nasync fn live() {}\n"
    assert unignored(source) == ["live (line 4)"]


# --- features ----------------------------------------------------------------

CRATE_CFG = re.compile(r"^#!\[cfg\((.*)\)\]", re.M)
FEATURE = re.compile(r"feature\s*=\s*\"([^\"]+)\"")


def gated_features(source: str) -> set[str]:
    """The features a file's crate-level `#![cfg(...)]` attributes name."""
    return {f for cfg in CRATE_CFG.findall(source) for f in FEATURE.findall(cfg)}


def feature_problems(features, required, source: str) -> list[str]:
    problems = []
    if extra := set(features) - set(required):
        problems.append(f"declares {sorted(extra)}, which required-features lacks")
    if missing := set(required) - set(features):
        problems.append(f"needs {sorted(missing)} but does not declare it in `features`")
    if gated := gated_features(source) - set(required):
        problems.append(f"is #![cfg]-gated on {sorted(gated)} without required-features, so "
                        f"a build without it compiles an empty binary that runs no tests")
    return problems


@pytest.mark.parametrize("key", sorted(live_targets(METADATA)), ids="/".join)
def test_live_target_features_match_required_features(key: tuple[str, str]) -> None:
    target = live_targets(METADATA)[key]
    live = _entry(key)
    problems = feature_problems(live.features, target.get("required-features") or [],
                                _path(*key).read_text())
    assert not problems, f"{key[0]} {key[1]}: {'; '.join(problems)}"


def test_a_target_missing_required_features_is_caught() -> None:
    """clob's `live_ws` before its `[[test]]` stanza: gated on `ws`, required nothing."""
    gated = '//! docs\n\n#![cfg(feature = "ws")]\n\nuse x;\n'
    assert feature_problems([], [], gated) == [
        "is #![cfg]-gated on ['ws'] without required-features, so a build without it "
        "compiles an empty binary that runs no tests"]
    assert feature_problems(["ws"], ["ws"], gated) == []
    assert feature_problems(["ws"], [], "") == ["declares ['ws'], which required-features lacks"]
    assert feature_problems([], ["ws"], "") == [
        "needs ['ws'] but does not declare it in `features`"]


def test_gated_features_reads_every_feature_in_a_cfg() -> None:
    assert gated_features('#![cfg(all(feature = "ws", feature = "keychain"))]\n') == {
        "ws", "keychain"}
    assert gated_features('#[cfg(feature = "ws")]\nfn f() {}\n') == set()


# --- secrets -----------------------------------------------------------------


@pytest.mark.parametrize("key", sorted(live_targets(METADATA)), ids="/".join)
def test_scanned_env_names_equal_declared_secrets(key: tuple[str, str]) -> None:
    scanned = gen_registry.env_names(_source(*key))
    declared = set(_entry(key).secrets)
    assert scanned == declared, (
        f"{key[0]} {key[1]}.rs reads {sorted(scanned - declared)} without declaring them "
        f"in `secrets`, and declares {sorted(declared - scanned)} it does not read"
    )


LITERAL_SECRETS = [
    (crate, target, name)
    for (crate, target), live in sorted(registered(REGISTRY).items())
    for name in live.secrets
    if f'"{name}"' in _source(crate, target)
]


def test_some_secrets_are_read_through_literals() -> None:
    """Guards the mutation test below against an empty parameter list."""
    assert len(LITERAL_SECRETS) >= 10


@pytest.mark.parametrize(("crate", "target", "name"), LITERAL_SECRETS,
                         ids=[f"{c}/{t}/{n}" for c, t, n in LITERAL_SECRETS])
def test_renaming_one_env_literal_fails_the_check(crate: str, target: str, name: str) -> None:
    source = _source(crate, target)
    declared = set(registered(REGISTRY)[(crate, target)].secrets)
    mutated = gen_registry.env_names(source.replace(f'"{name}"', f'"{name}_RENAMED"'))
    assert mutated != declared
    assert f"{name}_RENAMED" in mutated


@pytest.mark.parametrize("key", sorted(live_targets(METADATA)), ids="/".join)
def test_no_live_target_reads_the_environment_but_through_the_loaders(key: tuple[str, str]) -> None:
    reads = gen_registry.direct_env_reads(_source(*key))
    assert not reads, (
        f"{key[0]} {key[1]}.rs reads the environment around polyoxide-test-support's "
        f"loaders, where the secrets check cannot see the names: {reads}. Use `load_env`, "
        f"`optional_env` or `keychain`.")


def test_a_direct_env_read_is_found() -> None:
    source = """
//! Prose may name `std::env::var` and `dotenvy::dotenv()`.
// let a = std::env::var("COMMENTED_OUT");
let s = "std::env::var(\"IN_A_STRING\")";
let a = std::env::var("A_KEY").ok();
let b = env::var_os("B_KEY");
use std::env::vars;
dotenvy::dotenv().ok();
let account = Account::from_env().unwrap();
let c = some_var("NOT_ENV");
let d = Config::from_env_or_default();
use std::env::{self, var};
let e = env!("E_KEY");
let f = option_env!("F_KEY");
let g = polyoxide_test_support::load_env(&["G_KEY"]);
let h = my_env::thing();
let i = crate::env::thing();
"""
    assert [read.split(":")[0] for read in gen_registry.direct_env_reads(source)] == [
        "line 5", "line 6", "line 7", "line 8", "line 9", "line 12", "line 13", "line 14"]


def test_the_scan_reads_only_the_loaders() -> None:
    """A direct read names nothing the scan counts; the test above refuses it."""
    source = """
let a = std::env::var("A_KEY").ok();
let b = env::var_os("B_KEY");
let d = option_env!("D_KEY");
let f = Symbol::new("BTCUSDT");
let g = optional_env("G_KEY");
"""
    assert gen_registry.env_names(source) == {"G_KEY"}


def test_the_scan_reads_the_credential_loaders_arguments() -> None:
    source = """
use polyoxide_test_support::{keychain, load_env, optional_env};

let creds = load_env(&[
    "A_KEY",
    "B_KEY",
])
.or_else(|_| polyoxide_test_support::keychain(SERVICE, &[("A_KEY", "a"), ("B_KEY", key()),]))
.unwrap_or_else(|missing| missing.or_auth_gated());
let c = optional_env("C_KEY");
let d = keychain("svc", &[("D_KEY", "d")]);
"""
    assert gen_registry.env_names(source) == {"A_KEY", "B_KEY", "C_KEY", "D_KEY"}


def test_the_scan_skips_what_only_looks_like_a_loader_call() -> None:
    source = """
// load_env(NOT_A_CALL)
/// optional_env(also_prose)
pub fn load_env(names: &[&str]) -> Result<Creds, Missing> { todo!() }
fn keychain(service: &str) {}
let a = Account::from_keychain();
let b = polyoxide_core::keychain::get("svc", "key");
let c = store.keychain(service);
#[cfg(feature = "keychain")]
fn reload_env() {}
"""
    assert gen_registry.env_names(source) == set()


def test_a_loader_name_in_a_string_or_a_comment_is_not_a_call() -> None:
    source = """
let a = optional_env("A_KEY").expect("keychain (macOS) or load_env (CI)");
let b = r#"optional_env(raw)"#; // keychain (old) and load_env(NAMES)
/* optional_env(name) */
let creds = load_env(&[ // the keys (both)
    "B_KEY", /* , */ "C_KEY",
]);
"""
    assert gen_registry.env_names(source) == {"A_KEY", "B_KEY", "C_KEY"}


@pytest.mark.parametrize("call", [
    "load_env(NAMES)",
    "load_env(&[NAME])",
    'load_env(&["A_KEY", name])',
    "optional_env(name)",
    'optional_env(&format!("{PREFIX}_KEY"))',
    'keychain("svc", ENTRIES)',
    'keychain("svc", &[(NAME, "k")])',
    'keychain("svc", &["A_KEY"])',
    'load_env(&["A_KEY"], extra)',
])
def test_a_loader_call_without_literal_names_is_refused(call: str) -> None:
    with pytest.raises(gen_registry.RegistryError, match="loader|literal|slice|pair"):
        gen_registry.env_names(f"let creds = {call};\n")


def test_a_renamed_loader_argument_fails_the_check() -> None:
    """The mutation `test_renaming_one_env_literal_fails_the_check` makes, on a loader call."""
    declared = {"A_KEY", "B_KEY"}
    source = 'let creds = load_env(&["A_KEY", "B_KEY"]);\n'
    assert gen_registry.env_names(source) == declared
    assert gen_registry.env_names(source.replace('"B_KEY"', '"B_KEY_RENAMED"')) == {"A_KEY", "B_KEY_RENAMED"}


def _target_with_module(root: Path, module_path: str, module: str) -> Path:
    """`root/tests/live_x.rs` declaring `mod common;`, whose body is at `module_path`."""
    (root / "tests" / module_path).parent.mkdir(parents=True, exist_ok=True)
    (root / "tests" / module_path).write_text(module)
    target = root / "tests" / "live_x.rs"
    target.write_text("// mod commented;\nmod common;\n\n#[tokio::test]\n#[ignore]\nasync fn x() {}\n")
    return target


@pytest.mark.parametrize("module_path", ["common/mod.rs", "common.rs"])
def test_an_env_read_in_a_shared_module_must_be_declared(tmp_path: Path, module_path: str) -> None:
    target = _target_with_module(tmp_path, module_path,
                                 'pub fn key() -> Option<String> { optional_env("SHARED_KEY") }\n')
    assert gen_registry.module_files(target) == [target, tmp_path / "tests" / module_path]
    assert gen_registry.env_names(target.read_text()) == set()
    assert gen_registry.env_names(gen_registry.target_source(target)) == {"SHARED_KEY"}


def test_an_unignored_test_in_a_shared_module_is_caught(tmp_path: Path) -> None:
    target = _target_with_module(tmp_path, "common/mod.rs", "#[tokio::test]\nasync fn shared() {}\n")
    found = {file.name: unignored(file.read_text()) for file in gen_registry.module_files(target)}
    assert found == {"live_x.rs": [], "mod.rs": ["shared (line 2)"]}


def test_the_real_shared_modules_are_scanned() -> None:
    """data's live suite declares `mod common;`. binance's did until its wire
    helpers moved to polyoxide-test-support."""
    for key in [("polyoxide-data", "live_api")]:
        files = gen_registry.module_files(_path(*key))
        assert [f.relative_to(_path(*key).parent).as_posix() for f in files[1:]] == ["common/mod.rs"]
