"""Unit tests for scripts/api_removals.py.

The report parser and the compile test's error reader run on output captured from
cargo-semver-checks 0.51.0 and cargo under `fixtures/semver-checks/` (see its
PROVENANCE.md). Everything that would run git, cargo or cargo-semver-checks runs a
stub instead, and the S2 gate's two path listings are given as data, so nothing
here builds a crate. The tests of the real tree read docs/s1-removals.md,
docs/rename-manifest.md, the workflows and `cargo metadata --offline`.
"""

from __future__ import annotations

import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[3]
FIXTURES = Path(__file__).resolve().parent / "fixtures" / "semver-checks"
CAPTURED_ROOT = "/home/runner/work/polyoxide/polyoxide"


def _load(name: str):
    """A module from `scripts/`, which lives outside this uv project."""
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


api_removals = _load("api_removals")
RemovalsError = api_removals.RemovalsError
Lint = api_removals.Lint
HiddenPath = api_removals.HiddenPath

CRATES = ["polyoxide", "polyoxide-binance", "polyoxide-cli", "polyoxide-clob", "polyoxide-core",
          "polyoxide-data", "polyoxide-gamma", "polyoxide-perps", "polyoxide-relay",
          "polyoxide-rtds", "polyoxide-sports"]
# Each crate in the directory of its own name, as in the real workspace.
DIRS = {name: name for name in CRATES}


def captured(name: str) -> str:
    return (FIXTURES / name).read_text()


# --- the captured reports ----------------------------------------------------

# cargo-semver-checks 0.51.0 on two crates against v0.38.1, with
# `--release-type patch`, after four scratch edits. See PROVENANCE.md.
MIXED = [
    Lint("polyoxide-rtds", "function_missing", "failure",
         "function polyoxide_rtds::decode::decode_plain", "src/decode.rs"),
    Lint("polyoxide-rtds", "struct_marked_non_exhaustive", "failure",
         "struct DisplayPoint", "src/payload.rs"),
    Lint("polyoxide-sports", "inherent_method_missing", "failure",
         "MatchUpdate::key", "src/update.rs"),
    Lint("polyoxide-sports", "inherent_method_must_use_added", "failure",
         "method polyoxide_sports::supervised::SportsWsBuilder::new", "src/supervised.rs"),
    Lint("polyoxide-sports", "inherent_method_must_use_added", "failure",
         "method polyoxide_sports::SportsWsBuilder::new", "src/supervised.rs"),
]


def test_a_mixed_report_yields_each_item_keyed_by_crate_lint_and_file() -> None:
    report = api_removals.parse_report(captured("mixed.txt"), DIRS)
    assert report.checked == ["polyoxide-rtds", "polyoxide-sports"]
    assert report.lints == MIXED
    assert MIXED[2].key == "polyoxide-sports inherent_method_missing: MatchUpdate::key (src/update.rs)"


def test_only_removal_lints_are_removals() -> None:
    report = api_removals.parse_report(captured("mixed.txt"), DIRS)
    assert [lint.lint for lint in report.lints if lint.is_removal] == [
        "function_missing", "inherent_method_missing"]


def test_an_item_printed_once_per_path_is_kept_once() -> None:
    """`MatchUpdate` is importable at two paths, and the template names only the
    type, so the tool prints the same line twice."""
    text = captured("mixed.txt")
    assert text.count("  MatchUpdate::key, previously in file") == 2
    keys = [lint.key for lint in api_removals.parse_report(text, DIRS).lints]
    assert keys.count(MIXED[2].key) == 1


def test_same_named_items_in_one_crate_keep_apart() -> None:
    """v1 and v2 of polyoxide-data each have a `ListTrades`. The tool's item text is
    the same for both, and only the file tells the two removals apart."""
    report = api_removals.parse_report(captured("twins.txt"), DIRS)
    assert [lint.key for lint in report.lints] == [
        "polyoxide-data inherent_method_missing: ListTrades::limit (src/v2/api/feeds.rs)",
        "polyoxide-data inherent_method_missing: ListTrades::limit (src/api/trades.rs)",
    ]


def test_keys_carry_the_crate_relative_file_and_no_line() -> None:
    """A key must not hold the baseline's extraction directory or the checkout, which
    differ between machines, or a line number, which an edit above the item moves."""
    text = captured("mixed.txt") + captured("twins.txt")
    assert "/semver-checks/git-v0_38_1/" in text and f"{CAPTURED_ROOT}/polyoxide-rtds/" in text
    for lint in api_removals.parse_report(text, DIRS).lints:
        assert lint.file.startswith("src/") and lint.file.endswith(".rs"), lint.key
        assert re.search(r":\d+", lint.key) is None, lint.key


def test_a_clean_report_names_every_crate_and_no_lint() -> None:
    report = api_removals.parse_report(captured("clean.txt"), DIRS)
    assert report.lints == []
    assert report.checked == CRATES


def test_colour_codes_are_ignored() -> None:
    coloured = captured("mixed.txt").replace("--- failure", "\x1b[1m\x1b[31m--- failure")
    coloured = coloured.replace("Failed in:", "\x1b[1mFailed in:\x1b[0m")
    assert api_removals.parse_report(coloured, DIRS).lints == MIXED


# One line per template the removal lints use, from the 0.51.0 `.ron` files, and
# items named like the words the templates use.
@pytest.mark.parametrize(("line", "item", "path"), [
    ("function polyoxide_x::f, previously in file /a/b/src/lib.rs:45",
     "function polyoxide_x::f", "/a/b/src/lib.rs"),
    ("Foo::bar, previously in file src/lib.rs:7", "Foo::bar", "src/lib.rs"),
    ("field x of struct Foo, previously in file src/lib.rs:7", "field x of struct Foo", "src/lib.rs"),
    ("field Foo.x previously in file src/lib.rs:7", "field Foo.x", "src/lib.rs"),
    ("enum Foo in file src/lib.rs:7", "enum Foo", "src/lib.rs"),
    ("macro foo in src/lib.rs:7", "macro foo", "src/lib.rs"),
    ("macro #[foo] in src/lib.rs:7", "macro #[foo]", "src/lib.rs"),
    ("Foo::BAR, previously at src/lib.rs:7", "Foo::BAR", "src/lib.rs"),
    ("associated type Tr::Item, previously at src/lib.rs:7", "associated type Tr::Item",
     "src/lib.rs"),
    ("feature ws in the package's Cargo.toml", "feature ws in the package's Cargo.toml", None),
    ("field at of struct Foo, previously in file src/lib.rs:7", "field at of struct Foo",
     "src/lib.rs"),
    ("field in of struct Foo, previously in file src/lib.rs:7", "field in of struct Foo",
     "src/lib.rs"),
    ("at in file src/at.rs:3", "at", "src/at.rs"),
    ("Foo::at, previously at src/lib.rs:7", "Foo::at", "src/lib.rs"),
])
def test_each_location_template_is_split_from_its_item(line: str, item: str, path) -> None:
    assert api_removals.split_location(line) == (item, path)


@pytest.mark.parametrize(("path", "crate_dir", "file"), [
    (f"{CAPTURED_ROOT}/target/semver-checks/git-v0_38_1/1d045c7f/polyoxide-data/src/api/trades.rs",
     "polyoxide-data", "src/api/trades.rs"),
    (f"{CAPTURED_ROOT}/polyoxide-rtds/src/payload.rs", "polyoxide-rtds", "src/payload.rs"),
    (f"{CAPTURED_ROOT}/polyoxide/src/lib.rs", "polyoxide", "src/lib.rs"),
    ("crates/core/src/lib.rs", "crates/core", "src/lib.rs"),
    # A crate the baseline kept elsewhere keeps the baseline's directory.
    (f"{CAPTURED_ROOT}/target/semver-checks/git-v0_38_1/1d045c7f/old-core/src/lib.rs",
     "polyoxide-core", "old-core/src/lib.rs"),
])
def test_a_path_is_made_relative_to_its_crate(path: str, crate_dir: str, file: str) -> None:
    assert api_removals.crate_file(path, crate_dir) == file


@pytest.mark.parametrize(("lint", "removal"), [
    ("function_missing", True),
    ("inherent_method_missing", True),
    ("feature_missing", True),
    ("module_missing", True),
    ("crate_missing", True),
    ("macro_no_longer_exported", True),
    ("struct_now_doc_hidden", True),
    ("trait_removed_associated_type", True),
    ("trait_removed_associated_constant", True),
    ("struct_marked_non_exhaustive", False),
    ("function_parameter_count_changed", False),
    ("enum_variant_added", False),
    ("trait_removed_supertrait", False),
    ("missing_something_new", False),
])
def test_which_lints_are_removals(lint: str, removal: bool) -> None:
    assert Lint("c", lint, "failure", "x").is_removal is removal


def test_a_lint_before_any_crate_is_refused() -> None:
    text = "--- failure function_missing: pub fn removed or renamed ---\n\nFailed in:\n  function a::f\n"
    with pytest.raises(RemovalsError, match="before naming a crate"):
        api_removals.parse_report(text)


def test_a_lint_whose_items_cannot_be_read_is_refused() -> None:
    """If the item lines changed shape, reading on would let a removal through unnamed."""
    text = captured("mixed.txt").replace("\n  MatchUpdate::key,", "\nMatchUpdate::key,")
    with pytest.raises(RemovalsError, match="polyoxide-sports inherent_method_missing"):
        api_removals.parse_report(text)


# --- docs/s1-removals.md -----------------------------------------------------

LISTING = """---
baseline: v1.0.0
---

# Removals

Prose, and a list that is not an entry:

## Removed

- `polyoxide-a function_missing: function polyoxide_a::f (src/lib.rs)` Story 2.1: use `g`.
- `` polyoxide-a struct_missing: struct polyoxide_a::`Odd` `` a key ending in a backtick

## Doc-hidden paths consumers import

Some prose.

- `polyoxide_a::fixtures::{ONE, TWO}` `test-server`
- `polyoxide_a::ws::frame_for_tests` `ws` `test-server`
- `polyoxide_b::Alias`
- `polyoxide_b::old_fixtures` `test-server` **Removed** by Story 2.3: use `polyoxide_c::x`.

## Notes

- `not::an::entry`
"""


def test_the_listing_is_read() -> None:
    listing = api_removals.parse_listing(LISTING)
    assert listing.baseline == "v1.0.0"
    assert listing.removed == {
        "polyoxide-a function_missing: function polyoxide_a::f (src/lib.rs)",
        "polyoxide-a struct_missing: struct polyoxide_a::`Odd`",
    }
    assert listing.hidden == (
        HiddenPath("polyoxide_a::fixtures::ONE", ("test-server",)),
        HiddenPath("polyoxide_a::fixtures::TWO", ("test-server",)),
        HiddenPath("polyoxide_a::ws::frame_for_tests", ("ws", "test-server")),
        HiddenPath("polyoxide_b::Alias"),
        HiddenPath("polyoxide_b::old_fixtures", ("test-server",), removed=True),
    )


def test_an_empty_removed_section_lists_nothing() -> None:
    """Prose in a section, code spans included, is not an entry."""
    head, _, rest = LISTING.partition("## Removed\n")
    _, _, tail = rest.partition("## Doc-hidden")
    text = head + "## Removed\n\nNone yet. `polyoxide-a function_missing: x` is prose.\n\n## Doc-hidden" + tail
    listing = api_removals.parse_listing(text)
    assert listing.removed == frozenset()
    assert len(listing.hidden) == 5


@pytest.mark.parametrize(("edit", "message"), [
    (lambda t: t.replace("baseline: v1.0.0\n", ""), "no `baseline:`"),
    (lambda t: t.removeprefix("---\n"), "must start with front matter"),
    (lambda t: t.replace("## Removed", "## Removals"), "no `## Removed` section"),
    (lambda t: t.replace("## Doc-hidden paths consumers import", "## Hidden"),
     "no `## Doc-hidden paths consumers import` section"),
    (lambda t: t.replace("- `polyoxide_b::Alias`", "- polyoxide_b::Alias"),
     "must start with a crate path"),
    (lambda t: t.replace("- `polyoxide_b::Alias`", "- `Alias`"), "must start with a crate path"),
    (lambda t: t.replace("{ONE, TWO}", "{ONE, two::THREE}"), "must group plain names"),
    (lambda t: t.replace("{ONE, TWO}", "{ONE, }"), "must group plain names"),
    (lambda t: t.replace("`ws` `test-server`", "`ws` `two words`"), "not feature names"),
    (lambda t: t.replace("- `polyoxide-a function_missing", "- polyoxide-a `function_missing"),
     "must start with its key in backticks"),
])
def test_a_malformed_listing_is_refused(edit, message: str) -> None:
    """A renamed heading must not quietly turn the compile test into a no-op."""
    with pytest.raises(RemovalsError, match=message):
        api_removals.parse_listing(edit(LISTING))


@pytest.mark.parametrize("entry", [
    "- `polyoxide-a function_missing: function polyoxide_a::f (src/lib.rs)`",
    "- `polyoxide-a function_missing: function polyoxide_a::f (src/lib.rs)`   ",
])
def test_a_removed_entry_must_say_what_replaces_it(entry: str) -> None:
    """prader-rs migrates from this list, so a bare key tells it nothing."""
    text = LISTING.replace(
        "- `polyoxide-a function_missing: function polyoxide_a::f (src/lib.rs)` Story 2.1: use `g`.",
        entry)
    with pytest.raises(RemovalsError, match="must say, after its key"):
        api_removals.parse_listing(text)


# --- a fixture workspace -----------------------------------------------------


class Stub:
    """Answers each command by its leading words, recording every call."""

    def __init__(self, answers: dict[tuple[str, ...], tuple[int, str]]) -> None:
        self.answers = answers
        self.calls: list[list[str]] = []

    def __call__(self, command: list[str]) -> subprocess.CompletedProcess:
        self.calls.append(command)
        for prefix, answer in self.answers.items():
            if tuple(command[: len(prefix)]) == prefix:
                if isinstance(answer, BaseException):
                    raise answer
                code, out = answer(command) if callable(answer) else answer
                return subprocess.CompletedProcess(command, code, out, "")
        raise AssertionError(f"unexpected command {command}")

    def called(self, *prefix: str) -> list[list[str]]:
        return [c for c in self.calls if tuple(c[: len(prefix)]) == prefix]


def workspace(root: Path, *extra: str, version: str = "0.38.1") -> dict:
    """`cargo metadata` for the real crate names plus `extra`, and an unpublished
    polyoxide-py."""
    def package(name: str, publish: list | None = None) -> dict:
        return {"name": name, "version": version, "id": f"path+file://{root}/{name}#{version}",
                "publish": publish, "manifest_path": str(root / name / "Cargo.toml"),
                "dependencies": [], "targets": [{"kind": ["lib"], "name": name}]}
    packages = [package(n) for n in [*CRATES, *extra]] + [package("polyoxide-py", [])]
    return {"packages": packages, "workspace_members": [p["id"] for p in packages],
            "target_directory": str(root / "target")}


def baseline_git(members: dict[str, tuple[str, str]], rev: str = "v0.38.1") -> dict:
    """`git show` answers for a baseline workspace: directory -> (name, publish line)."""
    lines = ["[workspace]", "members = [" + ", ".join(f'"{d}"' for d in members) + "]"]
    answers = {("git", "show", f"{rev}:Cargo.toml"): (0, "\n".join(lines) + "\n")}
    for directory, (name, publish) in members.items():
        answers[("git", "show", f"{rev}:{directory}/Cargo.toml")] = (
            0, f'[package]\nname = "{name}"\nversion = "0.38.1"\n{publish}\n')
    return answers


BASELINE = {name: (name, "") for name in CRATES} | {"polyoxide-py": ("polyoxide-py", "publish = false")}


def write_stage(root: Path, stage: str) -> None:
    (root / "Cargo.toml").write_text(f'[workspace.metadata.polyoxide]\nstage = "{stage}"\n')


def write_listing(root: Path, removed: list[str] = (), hidden: list[str] = (),
                  baseline: str = "v0.38.1", listing: str = "docs/s1-removals.md") -> None:
    lines = ["---", f"baseline: {baseline}", "---", "", "## Removed", ""]
    lines += [f"- `{key}` Story 9.9: use something else." for key in removed]
    lines += ["", "## Doc-hidden paths consumers import", ""]
    lines += [f"- {entry}" for entry in hidden]
    (root / listing).parent.mkdir(parents=True, exist_ok=True)
    (root / listing).write_text("\n".join(lines) + "\n")
    if not (root / "Cargo.toml").exists():
        write_stage(root, "S1")


def runners(root: Path, report: tuple[int, str], *, extra: tuple[str, ...] = (),
            baseline: dict | None = None, compiled=(0, ""),
            version: str = "0.38.1", rev: str = "v0.38.1") -> tuple[Stub, Stub]:
    metadata = workspace(root, *extra, version=version)
    run = Stub({
        ("cargo", "metadata"): (0, json.dumps(metadata)),
        ("git", "rev-parse"): (0, "1" * 40 + "\n"),
        **baseline_git(BASELINE if baseline is None else baseline, rev),
    })
    stream = Stub({("cargo", "semver-checks"): report, ("cargo", "check"): compiled})
    return run, stream


def mixed_and_clean() -> str:
    """The mixed report followed by the clean report's other nine crates."""
    clean = captured("clean.txt")
    keep = [block for block in clean.split("    Building ")
            if not block.startswith(("polyoxide-rtds ", "polyoxide-sports "))]
    return captured("mixed.txt") + "    Building ".join(keep)


def check(root: Path, run: Stub, stream: Stub) -> int:
    return api_removals.main(["check", "--baseline", "v0.38.1"], root, run, stream)


REMOVAL_KEYS = [lint.key for lint in MIXED if lint.is_removal]


# --- exit codes --------------------------------------------------------------


def test_exit_100_is_data(tmp_path: Path) -> None:
    write_listing(tmp_path, removed=REMOVAL_KEYS)
    run, stream = runners(tmp_path, (100, mixed_and_clean()))
    assert check(tmp_path, run, stream) == 0


@pytest.mark.parametrize("code", [101, 1, -9])
def test_any_other_exit_is_never_read_as_clean(tmp_path: Path, code: int,
                                               capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (code, captured("clean.txt")))
    assert check(tmp_path, run, stream) == 2
    assert f"exited {code}, which is an error of its own" in capsys.readouterr().err
    assert not stream.called("cargo", "check")


def test_exit_100_with_no_failure_read_is_refused(tmp_path: Path,
                                                  capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (100, captured("clean.txt")))
    assert check(tmp_path, run, stream) == 2
    assert "exited 100, but no failure could be read" in capsys.readouterr().err


def test_exit_0_with_a_failure_read_is_refused(tmp_path: Path) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, mixed_and_clean()))
    assert check(tmp_path, run, stream) == 2


def test_a_crate_the_tool_never_names_is_refused(tmp_path: Path,
                                                 capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    report = captured("clean.txt").replace("Checking polyoxide-gamma ", "Skipping polyoxide-gamma ")
    run, stream = runners(tmp_path, (0, report))
    assert check(tmp_path, run, stream) == 2
    assert "never named polyoxide-gamma" in capsys.readouterr().err


@pytest.mark.parametrize(("where", "error"), [
    ("cargo-semver-checks", OSError(28, "No space left on device")),
    ("cargo-metadata", None),
])
def test_an_environment_failure_cannot_decide(tmp_path: Path, where: str, error,
                                              capsys: pytest.CaptureFixture[str]) -> None:
    """A full disk or garbled JSON says nothing about the API, so it is exit 2, not
    the gate's own 1 and not a traceback."""
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    if where == "cargo-semver-checks":
        stream.answers[("cargo", "semver-checks")] = error
    else:
        run.answers[("cargo", "metadata")] = (0, '{"packages": [')
    assert check(tmp_path, run, stream) == 2
    assert "::error::cannot decide" in capsys.readouterr().err


# --- listed and unlisted removals --------------------------------------------


def test_an_unlisted_removal_fails_naming_it(tmp_path: Path,
                                             capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, removed=[MIXED[0].key])
    run, stream = runners(tmp_path, (100, mixed_and_clean()))
    assert check(tmp_path, run, stream) == 1
    err = capsys.readouterr().err
    assert f"  {MIXED[2].key}\n" in err
    assert "decode_plain" not in err.split("::error::")[1]


def test_a_key_without_its_file_does_not_list_the_removal(tmp_path: Path) -> None:
    """A key from before files were part of it, or a twin's key, is a different key."""
    write_listing(tmp_path, removed=[
        MIXED[0].key, "polyoxide-sports inherent_method_missing: MatchUpdate::key"])
    run, stream = runners(tmp_path, (100, mixed_and_clean()))
    assert check(tmp_path, run, stream) == 1


def test_other_lints_are_allowed(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """S1 changes error and builder shapes on purpose; only removals are gated."""
    write_listing(tmp_path, removed=REMOVAL_KEYS)
    run, stream = runners(tmp_path, (100, mixed_and_clean()))
    assert check(tmp_path, run, stream) == 0
    assert "2 removals (2 listed), and 3 other changes" in capsys.readouterr().err


def test_a_listed_removal_no_longer_reported_is_a_warning(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, removed=["polyoxide-core struct_missing: struct polyoxide_core::Gone"])
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    assert check(tmp_path, run, stream) == 0
    assert "::warning::" in capsys.readouterr().err


def test_check_passes_release_type_patch_and_no_colour(tmp_path: Path) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    check(tmp_path, run, stream)
    assert stream.called("cargo", "semver-checks") == [[
        "cargo", "semver-checks", "--workspace", "--baseline-rev", "v0.38.1",
        "--release-type", "patch", "--color", "never"]]


def test_the_listing_must_name_the_same_baseline(tmp_path: Path,
                                                 capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, baseline="v0.37.0")
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    assert check(tmp_path, run, stream) == 2
    assert "against v0.37.0, not v0.38.1" in capsys.readouterr().err
    assert stream.calls == []


def test_a_baseline_missing_from_the_checkout_is_refused(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    run.answers[("git", "rev-parse")] = (1, "")
    assert check(tmp_path, run, stream) == 2
    assert "is not a commit in this checkout" in capsys.readouterr().err
    assert stream.calls == []


# --- the baseline's members --------------------------------------------------


def test_a_crate_absent_from_the_baseline_is_excluded_with_a_warning(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")), extra=("polyoxide-venue",))
    assert check(tmp_path, run, stream) == 0
    [command] = stream.called("cargo", "semver-checks")
    assert command[-2:] == ["--exclude", "polyoxide-venue"]
    assert ("::warning::polyoxide-venue is not a package in v0.38.1's workspace"
            in capsys.readouterr().err)


def test_nothing_is_excluded_when_every_crate_is_at_the_baseline(tmp_path: Path) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    check(tmp_path, run, stream)
    assert "--exclude" not in stream.called("cargo", "semver-checks")[0]


def test_a_crate_that_moved_is_still_checked(tmp_path: Path,
                                             capsys: pytest.CaptureFixture[str]) -> None:
    """Presence is by package name. By directory, a moved crate was excluded silently,
    and its removals went unchecked."""
    moved = {("crates/core" if d == "polyoxide-core" else d): v for d, v in BASELINE.items()}
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")), baseline=moved)
    assert check(tmp_path, run, stream) == 0
    assert "--exclude" not in stream.called("cargo", "semver-checks")[0]
    assert run.called("git", "show", "v0.38.1:crates/core/Cargo.toml")
    assert "::warning::" not in capsys.readouterr().err


def with_gone_crates() -> dict:
    return BASELINE | {"polyoxide-gone": ("polyoxide-gone", ""),
                       "polyoxide-private": ("polyoxide-private", "publish = false")}


def test_a_deleted_crate_is_a_removal_that_must_be_listed(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")), baseline=with_gone_crates())
    assert check(tmp_path, run, stream) == 1
    err = capsys.readouterr().err
    assert "  polyoxide-gone crate_missing\n" in err
    # An unpublished crate was never anyone's dependency.
    assert "polyoxide-private" not in err


def test_a_listed_deleted_crate_passes(tmp_path: Path) -> None:
    write_listing(tmp_path, removed=["polyoxide-gone crate_missing"])
    run, stream = runners(tmp_path, (0, captured("clean.txt")), baseline=with_gone_crates())
    assert check(tmp_path, run, stream) == 0


def test_publish_inherited_from_the_workspace_is_read(tmp_path: Path) -> None:
    answers = baseline_git({"a": ("polyoxide-a", "publish.workspace = true"),
                            "b": ("polyoxide-b", 'publish = ["crates-io"]'),
                            "c": ("polyoxide-c", "publish = []")})
    root = answers[("git", "show", "v0.38.1:Cargo.toml")][1]
    answers[("git", "show", "v0.38.1:Cargo.toml")] = (
        0, root + "\n[workspace.package]\npublish = false\n")
    members = api_removals.baseline_members("v0.38.1", Stub(answers))
    assert members == {"polyoxide-a": False, "polyoxide-b": True, "polyoxide-c": False}


def test_a_glob_member_at_the_baseline_cannot_be_read() -> None:
    answers = {("git", "show", "v0.38.1:Cargo.toml"): (0, '[workspace]\nmembers = ["crates/*"]\n')}
    with pytest.raises(RemovalsError, match="glob"):
        api_removals.baseline_members("v0.38.1", Stub(answers))


# --- the compile test --------------------------------------------------------

HIDDEN = ["`polyoxide_sports::fixtures::{SOCCER, CRICKET}` `test-server`",
          "`polyoxide_perps::ws::test_server` `test-server`",
          "`polyoxide_perps::ws::IncomingForTests` `ws`",
          "`polyoxide_perps::ws::frame_from_text_for_tests` `ws`",
          "`polyoxide_rtds::old` `test-fixtures` **Removed** by Story 2.4",
          "`polyoxide_clob::DynSigner`"]


def scratch_dir(root: Path, group: str) -> Path:
    return root / "target" / "api-removals" / group


def test_each_crate_and_feature_set_is_built_on_its_own(tmp_path: Path) -> None:
    """Built together, `ws` entries would compile through a `test-server` entry's
    feature; built apart, an entry that under-declares its features fails."""
    write_listing(tmp_path, hidden=HIDDEN)
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    assert check(tmp_path, run, stream) == 0
    builds = stream.called("cargo", "check")
    manifests = [Path(c[c.index("--manifest-path") + 1]) for c in builds]
    assert manifests == [scratch_dir(tmp_path, group) / "Cargo.toml" for group in [
        "polyoxide-sports+test-server", "polyoxide-perps+test-server", "polyoxide-perps+ws",
        "polyoxide-clob"]]
    target = str(tmp_path / "target" / "api-removals" / "target")
    for command in builds:
        assert command[command.index("--target-dir") + 1] == target
        assert command[-4:] == ["--message-format", "json", "--color", "never"]

    perps_ws = scratch_dir(tmp_path, "polyoxide-perps+ws")
    assert (perps_ws / "Cargo.toml").read_text().count("\npolyoxide-") == 1
    assert (f'polyoxide-perps = {{ path = "{tmp_path}/polyoxide-perps", features = ["ws"] }}'
            in (perps_ws / "Cargo.toml").read_text())
    assert [line for line in (perps_ws / "src/lib.rs").read_text().splitlines()
            if line.startswith("use ")] == [
        "use polyoxide_perps::ws::IncomingForTests as _;",
        "use polyoxide_perps::ws::frame_from_text_for_tests as _;"]
    sports = (scratch_dir(tmp_path, "polyoxide-sports+test-server") / "src/lib.rs").read_text()
    assert "use polyoxide_sports::fixtures::SOCCER as _;" in sports
    assert "use polyoxide_sports::fixtures::CRICKET as _;" in sports
    clob = (scratch_dir(tmp_path, "polyoxide-clob") / "Cargo.toml").read_text()
    assert f'polyoxide-clob = {{ path = "{tmp_path}/polyoxide-clob" }}' in clob
    assert "\n[workspace]\n" in clob
    assert not any("polyoxide-rtds" in str(m) for m in manifests)


def fails_for(group: str, output: str):
    """A `cargo check` answer that fails only the scratch crate of `group`."""
    def answer(command: list[str]) -> tuple[int, str]:
        failing = f"/{group}/Cargo.toml" in command[command.index("--manifest-path") + 1]
        return (101, output) if failing else (0, "")
    return answer


def unresolved(manifest: Path, line: int) -> str:
    """cargo's JSON message for an unresolved import at `line` of `manifest`'s crate."""
    return json.dumps({"reason": "compiler-message", "manifest_path": str(manifest),
                       "message": {"level": "error", "rendered": "error[E0432]",
                                   "spans": [{"file_name": "src/lib.rs", "line_start": line,
                                              "is_primary": True}]}}) + "\n"


def test_a_path_that_no_longer_compiles_fails_naming_it(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, hidden=HIDDEN)
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    check(tmp_path, run, stream)
    group = scratch_dir(tmp_path, "polyoxide-perps+ws")
    line = (group / "src/lib.rs").read_text().splitlines().index(
        "use polyoxide_perps::ws::frame_from_text_for_tests as _;") + 1
    stream.answers[("cargo", "check")] = fails_for(
        "polyoxide-perps+ws", unresolved(group / "Cargo.toml", line))
    capsys.readouterr()
    assert check(tmp_path, run, stream) == 1
    err = capsys.readouterr().err
    assert ("no longer compile from polyoxide-perps with `ws`: "
            "polyoxide_perps::ws::frame_from_text_for_tests.") in err
    # The other groups still built.
    assert len(stream.called("cargo", "check")) == 8


def test_an_unresolved_import_in_the_captured_build_is_read() -> None:
    manifest = Path(f"{CAPTURED_ROOT}/target/api-removals/polyoxide-sports+test-server/Cargo.toml")
    assert api_removals.error_lines(captured("compile-unresolved-import.txt"), manifest) == {13}


def test_an_error_in_a_dependency_is_never_read_as_a_listed_line() -> None:
    """The dependency's own error sits at its `src/lib.rs:5`, the very line where the
    scratch crate imports its first path. Matching `src/lib.rs:N` in the text blamed
    that path."""
    output = captured("compile-dependency-error.txt")
    manifest = Path(f"{CAPTURED_ROOT}/target/api-removals/polyoxide-sports+test-server/Cargo.toml")
    assert re.search(r"src/lib\.rs:5:1", output)
    assert api_removals.error_lines(output, manifest) == set()


def test_a_dependency_error_fails_without_blaming_a_path(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, hidden=HIDDEN)
    run, stream = runners(tmp_path, (0, captured("clean.txt")),
                          compiled=fails_for("polyoxide-sports+test-server",
                                             captured("compile-dependency-error.txt")))
    assert check(tmp_path, run, stream) == 1
    err = capsys.readouterr().err
    assert ("The compile test of polyoxide-sports with `test-server` failed, but not on a "
            "listed path") in err
    assert "no longer compile" not in err


def test_cargos_json_is_shown_as_rustc_rendered_it() -> None:
    message = {"reason": "compiler-message", "message": {"rendered": "error[E0432]: x\n"}}
    assert api_removals._echo(json.dumps(message)) == "error[E0432]: x\n"
    assert api_removals._echo('{"reason":"compiler-artifact","target":{}}') is None
    assert api_removals._echo("    Checking polyoxide-sports v0.38.1\n") == (
        "    Checking polyoxide-sports v0.38.1\n")


def test_a_listed_path_in_a_crate_that_is_gone_fails(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, hidden=["`polyoxide_gone::fixtures` `test-server`"])
    run, stream = runners(tmp_path, (0, captured("clean.txt")))
    assert check(tmp_path, run, stream) == 1
    assert "polyoxide_gone::fixtures" in capsys.readouterr().err
    assert not stream.called("cargo", "check")


def test_an_unlisted_removal_and_a_broken_path_are_both_reported(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path, hidden=HIDDEN)
    run, stream = runners(tmp_path, (100, mixed_and_clean()), compiled=(101, ""))
    assert check(tmp_path, run, stream) == 1
    err = capsys.readouterr().err
    assert "MatchUpdate::key" in err and "see cargo's output above" in err


# --- release mode ------------------------------------------------------------


def release(root: Path, run: Stub, stream: Stub, baseline: str = "v0.38.0") -> int:
    return api_removals.main(["release", "--baseline", baseline], root, run, stream)


def test_release_fails_on_any_lint_failure(tmp_path: Path,
                                           capsys: pytest.CaptureFixture[str]) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (100, mixed_and_clean()), rev="v0.38.0")
    assert release(tmp_path, run, stream) == 1
    err = capsys.readouterr().err
    for lint in MIXED:
        assert f"  {lint.key}\n" in err
    assert "a bump to 0.38.1 does not allow" in err


def test_release_lets_the_tool_infer_the_release_type(tmp_path: Path) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")), rev="v0.38.0")
    assert release(tmp_path, run, stream) == 0
    [command] = stream.called("cargo", "semver-checks")
    assert "--release-type" not in command
    assert command[:5] == ["cargo", "semver-checks", "--workspace", "--baseline-rev", "v0.38.0"]


def test_release_runs_the_compile_test(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """A patch release must not drop a type alias or a listed doc-hidden path either,
    and cargo-semver-checks sees neither."""
    write_listing(tmp_path, hidden=HIDDEN)
    run, stream = runners(tmp_path, (0, captured("clean.txt")), rev="v0.38.0",
                          compiled=fails_for("polyoxide-clob", ""))
    assert release(tmp_path, run, stream) == 1
    assert "The compile test of polyoxide-clob with default features failed" in (
        capsys.readouterr().err)
    assert len(stream.called("cargo", "check")) == 4


@pytest.mark.parametrize(("version", "status"), [("0.38.1", 1), ("0.39.0", 0), ("1.0.0", 0)])
def test_a_deleted_crate_needs_a_new_minor_in_a_release(tmp_path: Path, version: str,
                                                        status: int) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, captured("clean.txt")), baseline=with_gone_crates(),
                          version=version, rev="v0.38.0")
    assert release(tmp_path, run, stream) == status


@pytest.mark.parametrize(("baseline", "current", "allowed"), [
    ("v0.38.1", "0.38.2", False),
    ("v0.38.1", "0.39.0", True),
    ("v0.38.1", "1.0.0", True),
    ("v1.2.0", "1.3.0", False),
    ("v1.2.0", "2.0.0", True),
    ("12e83164", "0.39.0", False),
])
def test_only_a_new_0x_minor_or_major_may_break(baseline: str, current: str, allowed: bool) -> None:
    assert api_removals.raises_minor(baseline, current) is allowed


def test_release_refuses_an_error_exit(tmp_path: Path) -> None:
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (101, "error: no such baseline\n"), rev="v0.38.0")
    assert release(tmp_path, run, stream) == 2


# --- docs/rename-manifest.md -------------------------------------------------

Row = api_removals.Row
Snapshot = api_removals.public_api.Snapshot
PublicApiError = api_removals.public_api.PublicApiError

MANIFEST = """---
baseline: {baseline}
---

# Manifest

Prose naming `polyoxide_a::x` → `polyoxide_b::y`, which is not a row.

- `polyoxide_a::also_prose` → under no rows heading.

## Paths

{paths}
## Features

{features}
## Doc-hidden paths consumers import

{hidden}
## Unresolved entries

<!-- manifest:begin unresolved -->
{unresolved}<!-- manifest:end unresolved -->

## Stale rows

<!-- manifest:begin stale -->
{stale}<!-- manifest:end stale -->

## Changed in place

<!-- manifest:begin changed-in-place -->
{changed}<!-- manifest:end changed-in-place -->
"""


def manifest_text(paths=(), features=(), hidden=(), unresolved="None.\n", stale="None.\n",
                  changed="None.\n", baseline="v0.39.0") -> str:
    def lines(entries) -> str:
        return "".join(f"- {entry}\n" for entry in entries)
    return MANIFEST.format(baseline=baseline, paths=lines(paths), features=lines(features),
                           hidden=lines(hidden), unresolved=unresolved, stale=stale,
                           changed=changed)


def write_manifest(root: Path, *args, **kwargs) -> str:
    text = manifest_text(*args, **kwargs)
    (root / "docs").mkdir(parents=True, exist_ok=True)
    (root / "docs" / "rename-manifest.md").write_text(text)
    return text


def test_the_manifest_is_read() -> None:
    manifest = api_removals.parse_manifest(manifest_text(
        paths=["`polyoxide_clob` → `polyoxide_polymarket::clob` Story 5.4, feature `clob`.",
               "`polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` Story 5.3.",
               "`polyoxide_clob::ws::WebSocketError::ConnectionClosed` → removed: never "
               "constructed (Story 5.4).",
               "`polyoxide_a::Pair::0` → `polyoxide_b::Pair::0`"],
        features=["`polyoxide-clob/ws` → `polyoxide-polymarket/clob-ws` Story 5.4.",
                  "`polyoxide-gamma` → retired with the crate (Story 5.3)."],
        hidden=["`polyoxide_a::fixtures::{ONE, TWO}` `test-server`"],
        unresolved="- `polyoxide_a::x` function: no row\n"))
    assert manifest.baseline == "v0.39.0"
    assert manifest.rows == (
        Row("polyoxide_clob", "polyoxide_polymarket::clob", None),
        Row("polyoxide_data::DataApiError", "polyoxide_polymarket::data::DataError", None),
        Row("polyoxide_clob::ws::WebSocketError::ConnectionClosed", None,
            "removed: never constructed (Story 5.4)."),
        Row("polyoxide_a::Pair::0", "polyoxide_b::Pair::0", None),
    )
    assert manifest.features == (
        Row("polyoxide-clob/ws", "polyoxide-polymarket/clob-ws", None),
        Row("polyoxide-gamma", None, "retired with the crate (Story 5.3)."),
    )
    assert manifest.hidden == (HiddenPath("polyoxide_a::fixtures::ONE", ("test-server",)),
                               HiddenPath("polyoxide_a::fixtures::TWO", ("test-server",)))
    assert manifest.regions == {"unresolved": "- `polyoxide_a::x` function: no row\n",
                                "stale": "None.\n", "changed-in-place": "None.\n"}


def test_an_empty_manifest_has_no_rows() -> None:
    manifest = api_removals.parse_manifest(manifest_text())
    assert manifest.rows == manifest.features == manifest.hidden == ()


def test_a_regions_lines_are_never_rows() -> None:
    """A generated line looks like a row's start; it must not become one."""
    manifest = api_removals.parse_manifest(manifest_text(
        stale="- `polyoxide_a::x` → `polyoxide_b::x`\n"))
    assert manifest.rows == ()


@pytest.mark.parametrize(("edit", "message"), [
    (lambda t: t.replace("baseline: v0.39.0\n", ""), "no `baseline:`"),
    (lambda t: t.replace("## Paths", "## Path"), "no `## Paths` section"),
    (lambda t: t.replace("## Features", "## Feature"), "no `## Features` section"),
    (lambda t: t.replace("## Doc-hidden paths consumers import", "## Hidden"),
     "no `## Doc-hidden paths consumers import` section"),
    (lambda t: t.replace("<!-- manifest:end stale -->\n", ""), "manifest:end stale"),
    (lambda t: t + "<!-- manifest:begin unresolved -->\n", "once each"),
    (lambda t: t.replace("<!-- manifest:begin stale -->\n", "<!-- manifest:begin stale --> x\n"),
     "lines of their own"),
])
def test_a_malformed_manifest_is_refused(edit, message: str) -> None:
    with pytest.raises(RemovalsError, match=message):
        api_removals.parse_manifest(edit(manifest_text()))


@pytest.mark.parametrize(("row", "message"), [
    ("`polyoxide_a::Y` →", "must say, after `→`"),
    ("`polyoxide_a::Y` →   ", "must say, after `→`"),
    ("`polyoxide_a::Y` `polyoxide_b::Y`", "must follow it with `→`"),
    ("`polyoxide_a::Y` -> `polyoxide_b::Y`", "must follow it with `→`"),
    ("polyoxide_a::Y → `polyoxide_b::Y`", "must start with its old path"),
    ("`Y` → `polyoxide_b::Y`", "is not a path"),
    ("`polyoxide_a::{Y, Z}` → `polyoxide_b`", "is not a path"),
    ("`polyoxide_a::Y` → `not a path`", "which is not a path"),
])
def test_a_row_with_no_new_path_or_reason_cannot_decide(row: str, message: str) -> None:
    """prader-rs migrates from the rows, so a row that says nothing is refused."""
    with pytest.raises(RemovalsError, match=message) as raised:
        api_removals.parse_manifest(manifest_text(paths=[row]))
    assert raised.value.exit_code == 2


def test_a_feature_row_names_a_package_and_a_feature() -> None:
    with pytest.raises(RemovalsError, match="is not a crate or feature"):
        api_removals.parse_manifest(manifest_text(features=["`polyoxide_a::ws` → gone"]))


@pytest.mark.parametrize("section", ["Paths", "Features"])
def test_a_row_wrapped_onto_a_second_line_is_refused(section: str) -> None:
    """Only a row's first line is read, so a wrapped one would lose its end unseen."""
    row = ("`polyoxide-a/ws` → removed: the module" if section == "Features"
           else "`polyoxide_a::Y` → removed: the module")
    text = manifest_text(**{section.lower(): [row + "\n  went with its crate (Story 5.4)."]})
    with pytest.raises(RemovalsError, match="a row is one line") as raised:
        api_removals.parse_manifest(text)
    assert raised.value.exit_code == 2


def test_an_indented_line_elsewhere_is_prose() -> None:
    text = manifest_text(hidden=["`polyoxide_a::X`\n  continued prose"])
    assert api_removals.parse_manifest(text).hidden == (HiddenPath("polyoxide_a::X"),)


def test_an_old_path_has_one_row() -> None:
    with pytest.raises(RemovalsError, match="has two `## Paths` rows"):
        api_removals.parse_manifest(manifest_text(paths=["`polyoxide_a` → gone", "`polyoxide_a` → `b`"]))


@pytest.mark.parametrize(("path", "row"), [
    ("polyoxide_data::DataApiError::V2", "polyoxide_data::DataApiError"),
    ("polyoxide_data::DataApiError", "polyoxide_data::DataApiError"),
    ("polyoxide_data::DataApiErrorKind", "polyoxide_data"),
    ("polyoxide_data", "polyoxide_data"),
    ("polyoxide_database::X", None),
])
def test_the_longest_row_at_a_boundary_wins(path: str, row) -> None:
    rows = (Row("polyoxide_data", "polyoxide_polymarket::data", None),
            Row("polyoxide_data::DataApiError", "polyoxide_polymarket::data::DataError", None))
    found = api_removals.longest_row(rows, path)
    assert (found.old if found else None) == row


# --- the S2 gate -------------------------------------------------------------


def snap(*paths: str, features=(), hidden=()) -> Snapshot:
    """A listing from `"<kind> <path>"` strings."""
    listed = {}
    for entry in paths:
        kind, path = entry.split(" ")
        listed[(kind, path)] = path in hidden
    return Snapshot(listed, frozenset(features))


class Snapshots:
    """Answers `public_api.snapshot` with a listing per side."""

    def __init__(self, base: Snapshot, head: Snapshot) -> None:
        self.base, self.head = base, head
        self.calls: list[str | None] = []

    def __call__(self, rev, root, scratch, run, stream) -> Snapshot:
        self.calls.append(rev)
        assert scratch == root / "target" / "api-removals"
        if isinstance(self.base, BaseException):
            raise self.base
        return self.base if rev is not None else self.head


CLEAN = captured("clean.txt")


def gate(root: Path, base: Snapshot, head: Snapshot, *flags: str, report=(0, CLEAN),
         compiled=(0, "")):
    run, stream = runners(root, report, rev="v0.39.0", compiled=compiled)
    snapshots = Snapshots(base, head)
    status = api_removals.main(["manifest", "--baseline", "v0.39.0", *flags], root, run, stream,
                               snapshots)
    return status, run, stream, snapshots


def regenerated(root: Path) -> str:
    return (root / "target" / "api-removals" / "rename-manifest.md").read_text()


def region(text: str, name: str) -> str:
    return api_removals.split_regions(text)[1][name]


CLOB = snap("module polyoxide_clob", "struct polyoxide_clob::Clob",
            "function polyoxide_clob::Clob::new", "module polyoxide_clob::ws",
            "function polyoxide_clob::ws::gone", "function polyoxide_clob::fixtures",
            features=["polyoxide-clob/ws", "polyoxide-clob/default"],
            hidden={"polyoxide_clob::fixtures"})
MOVED = snap("module polyoxide_polymarket", "module polyoxide_polymarket::clob",
             "struct polyoxide_polymarket::clob::Clob",
             "function polyoxide_polymarket::clob::Clob::new",
             "module polyoxide_polymarket::clob::ws",
             features=["polyoxide-polymarket/clob-ws", "polyoxide-polymarket/default"])


def test_no_moves_reports_no_entry_and_regions_that_match(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    text = write_manifest(tmp_path)
    status, _, stream, snapshots = gate(tmp_path, CLOB, CLOB)
    assert status == 0
    assert snapshots.calls == ["v0.39.0", None]
    out, err = capsys.readouterr()
    assert "::warning::" not in err and out == ""
    assert "0 entries since v0.39.0: 0 resolved, 0 unresolved" in err
    assert "0 stale regions" in err
    assert regenerated(tmp_path) == text
    assert stream.called("cargo", "semver-checks") == [[
        "cargo", "semver-checks", "--workspace", "--baseline-rev", "v0.39.0",
        "--release-type", "patch", "--color", "never"]]


def test_a_crate_moved_under_a_rule_resolves_and_a_lost_path_points_nowhere(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    text = write_manifest(tmp_path, paths=["`polyoxide_clob` → `polyoxide_polymarket::clob` Story 5.4."],
                          features=["`polyoxide-clob/ws` → `polyoxide-polymarket/clob-ws`",
                                    "`polyoxide-clob/default` → `polyoxide-polymarket/default`"])
    status, *_ = gate(tmp_path, CLOB, MOVED)
    assert status == 0
    err = capsys.readouterr().err
    assert "8 entries since v0.39.0: 6 resolved, 2 unresolved (2 pointing nowhere)" in err
    assert region(regenerated(tmp_path), "unresolved") == (
        "- `polyoxide_clob::fixtures` function, doc-hidden: points nowhere, "
        "`polyoxide_polymarket::clob::fixtures`\n"
        "- `polyoxide_clob::ws::gone` function: points nowhere, "
        "`polyoxide_polymarket::clob::ws::gone`\n")
    assert "::warning::polyoxide_clob::ws::gone (function) is gone since v0.39.0" in err
    assert "::warning::docs/rename-manifest.md's `unresolved` region is stale" in err
    # Report-only: the checked-in manifest is untouched.
    assert (tmp_path / "docs" / "rename-manifest.md").read_text() == text


def test_strict_fails_on_an_unresolved_entry(tmp_path: Path) -> None:
    write_manifest(tmp_path, paths=["`polyoxide_clob` → `polyoxide_polymarket::clob`"])
    status, *_ = gate(tmp_path, CLOB, MOVED, "--write", "--strict")
    assert status == 1


def test_strict_passes_once_every_entry_resolves_and_the_regions_are_written(tmp_path: Path) -> None:
    write_manifest(tmp_path, paths=[
        "`polyoxide_clob` → `polyoxide_polymarket::clob`",
        "`polyoxide_clob::ws::gone` → removed: nothing called it (Story 5.4).",
        "`polyoxide_clob::fixtures` → removed: the fixtures moved to the kit."],
        features=["`polyoxide-clob` → retired with the crate; see `## Paths`."])
    assert gate(tmp_path, CLOB, MOVED, "--strict")[0] == 0


def test_an_items_own_row_overrides_its_crates_and_its_members_follow(
        tmp_path: Path) -> None:
    base = snap("module polyoxide_data", "enum polyoxide_data::DataApiError",
                "variant polyoxide_data::DataApiError::V2", "struct polyoxide_data::DataApi")
    head = snap("module polyoxide_polymarket", "module polyoxide_polymarket::data",
                "enum polyoxide_polymarket::data::DataError",
                "variant polyoxide_polymarket::data::DataError::V2",
                "struct polyoxide_polymarket::data::DataApi")
    write_manifest(tmp_path, paths=[
        "`polyoxide_data` → `polyoxide_polymarket::data` Story 5.3.",
        "`polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` Story 5.3."])
    status, *_ = gate(tmp_path, base, head, "--expand", str(tmp_path / "map.tsv"), "--strict")
    assert status == 0
    assert (tmp_path / "map.tsv").read_text().splitlines() == [
        "polyoxide_data\tpolyoxide_polymarket::data\tmodule\tresolved",
        "polyoxide_data::DataApi\tpolyoxide_polymarket::data::DataApi\tstruct\tresolved",
        "polyoxide_data::DataApiError\tpolyoxide_polymarket::data::DataError\tenum\tresolved",
        "polyoxide_data::DataApiError::V2\tpolyoxide_polymarket::data::DataError::V2\tvariant"
        "\tresolved",
    ]


def test_a_reason_row_covers_an_item_removed_outright_and_its_members(tmp_path: Path) -> None:
    base = snap("enum polyoxide_clob::WebSocketError",
                "variant polyoxide_clob::WebSocketError::ConnectionClosed",
                "struct_field polyoxide_clob::WebSocketError::ConnectionClosed::code")
    head = snap("enum polyoxide_clob::WebSocketError")
    write_manifest(tmp_path, paths=[
        "`polyoxide_clob::WebSocketError::ConnectionClosed` → removed: never constructed "
        "(Story 5.4)."])
    status, *_ = gate(tmp_path, base, head, "--strict", "--expand", str(tmp_path / "map.tsv"))
    assert status == 0
    assert (tmp_path / "map.tsv").read_text().splitlines() == [
        "polyoxide_clob::WebSocketError::ConnectionClosed\tremoved: never constructed "
        "(Story 5.4).\tvariant\tresolved",
        "polyoxide_clob::WebSocketError::ConnectionClosed::code\tremoved: never constructed "
        "(Story 5.4).\tstruct_field\tresolved",
    ]


def test_an_unrowed_entry_is_written_to_the_map_with_nothing_beside_it(tmp_path: Path) -> None:
    write_manifest(tmp_path)
    gate(tmp_path, snap("function polyoxide_a::f"), snap(), "--expand", str(tmp_path / "m.tsv"))
    assert (tmp_path / "m.tsv").read_text() == "polyoxide_a::f\t\tfunction\tno row\n"


def test_the_map_writes_a_target_only_when_it_resolved(tmp_path: Path) -> None:
    """A row that points nowhere is not a move a consumer can follow."""
    write_manifest(tmp_path, paths=["`polyoxide_clob` → `polyoxide_polymarket::clob`"])
    gate(tmp_path, CLOB, MOVED, "--expand", str(tmp_path / "m.tsv"))
    lines = (tmp_path / "m.tsv").read_text().splitlines()
    assert "polyoxide_clob::Clob\tpolyoxide_polymarket::clob::Clob\tstruct\tresolved" in lines
    assert "polyoxide_clob::ws::gone\t\tfunction\tpoints nowhere" in lines
    assert "polyoxide_clob::fixtures\t\tfunction\tpoints nowhere, doc-hidden" in lines


def test_the_map_escapes_a_reason_that_would_break_a_line_or_a_column(tmp_path: Path) -> None:
    write_manifest(tmp_path, paths=["`polyoxide_a::f` → gone\tfor good, see C:\\old"])
    gate(tmp_path, snap("function polyoxide_a::f"), snap(), "--expand", str(tmp_path / "m.tsv"))
    assert (tmp_path / "m.tsv").read_text() == (
        "polyoxide_a::f\tgone\\tfor good, see C:\\\\old\tfunction\tresolved\n")
    entry = api_removals.Entry("function", "polyoxide_a::f", False,
                               Row("polyoxide_a::f", None, "a\nb"), None, "resolved")
    assert api_removals.expand_line(entry) == "polyoxide_a::f\ta\\nb\tfunction\tresolved\n"


def test_a_changed_kind_is_an_entry(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Keyed by (kind, path): a struct that became an alias is not the same path."""
    write_manifest(tmp_path, paths=["`polyoxide_a::Ws` → `polyoxide_a::Ws`"])
    gate(tmp_path, snap("struct polyoxide_a::Ws"), snap("type_alias polyoxide_a::Ws"))
    assert ("- `polyoxide_a::Ws` struct: exists as type_alias at HEAD, `polyoxide_a::Ws`\n"
            in region(regenerated(tmp_path), "unresolved"))


def test_a_public_path_moved_onto_a_hidden_one_is_unresolved(tmp_path: Path) -> None:
    """Moving a path behind `#[doc(hidden)]` takes it from consumers; only a reason row
    of its own says so on purpose."""
    base = snap("function polyoxide_a::f", "function polyoxide_a::t", hidden={"polyoxide_a::t"})
    head = snap("function polyoxide_b::f", "function polyoxide_b::t",
                hidden={"polyoxide_b::f", "polyoxide_b::t"})
    write_manifest(tmp_path, paths=["`polyoxide_a` → `polyoxide_b`"])
    gate(tmp_path, base, head)
    assert region(regenerated(tmp_path), "unresolved") == (
        "- `polyoxide_a::f` function: maps to a doc-hidden path, `polyoxide_b::f`\n")
    write_manifest(tmp_path, paths=["`polyoxide_a` → `polyoxide_b`",
                                    "`polyoxide_a::f` → now doc-hidden: tests only (Story 9.9)."])
    assert gate(tmp_path, base, head, "--write", "--strict")[0] == 0


def test_a_feature_without_a_row_is_unresolved(tmp_path: Path) -> None:
    write_manifest(tmp_path)
    gate(tmp_path, snap(features=["polyoxide-rtds/test-fixtures"]),
         snap(features=["polyoxide-rtds/test-server"]))
    assert region(regenerated(tmp_path), "unresolved") == (
        "- `polyoxide-rtds/test-fixtures` feature: no row\n")


def test_a_feature_row_mapping_nowhere_is_unresolved(tmp_path: Path) -> None:
    write_manifest(tmp_path, features=["`polyoxide-rtds/test-fixtures` → `polyoxide-rtds/test`"])
    gate(tmp_path, snap(features=["polyoxide-rtds/test-fixtures"]),
         snap(features=["polyoxide-rtds/test-server"]))
    assert region(regenerated(tmp_path), "unresolved") == (
        "- `polyoxide-rtds/test-fixtures` feature: points nowhere, `polyoxide-rtds/test`\n")


def test_a_row_covering_no_entry_is_stale_and_says_why(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, paths=["`polyoxide_clob::Typo` → gone", "`polyoxide_clob` → gone"],
                   features=["`polyoxide-clob/wss` → gone", "`polyoxide-clob/ws` → gone"])
    status, *_ = gate(tmp_path, CLOB, CLOB)
    assert status == 0
    assert region(regenerated(tmp_path), "stale") == (
        "- `polyoxide_clob::Typo` in `## Paths`: covers no baseline path\n"
        "- `polyoxide_clob` in `## Paths`: covers no entry: every path it covers is still at "
        "HEAD\n"
        "- `polyoxide-clob/wss` in `## Features`: covers no baseline path\n"
        "- `polyoxide-clob/ws` in `## Features`: covers no entry: every path it covers is "
        "still at HEAD\n")
    err = capsys.readouterr().err
    assert "row for polyoxide_clob::Typo covers no baseline path" in err
    assert "row for polyoxide_clob covers no entry" in err
    write_manifest(tmp_path, paths=["`polyoxide_clob::Typo` → gone"],
                   stale="- `polyoxide_clob::Typo` in `## Paths`: covers no baseline path\n")
    assert gate(tmp_path, CLOB, CLOB, "--strict")[0] == 1


def test_a_row_covering_an_entry_is_not_stale(tmp_path: Path) -> None:
    write_manifest(tmp_path, paths=["`polyoxide_clob::ws` → gone"])
    gate(tmp_path, CLOB, snap(*[f"{k} {p}" for k, p in CLOB.paths if "::ws" not in p]))
    assert region(regenerated(tmp_path), "stale") == "None.\n"


def test_a_stale_region_is_diffed_and_write_rewrites_it(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, unresolved="- `polyoxide_clob::Old` struct: no row\n")
    status, *_ = gate(tmp_path, CLOB, CLOB, "--strict")
    assert status == 1
    out, err = capsys.readouterr()
    assert "-- `polyoxide_clob::Old` struct: no row\n+None.\n" in out
    assert "docs/rename-manifest.md (unresolved, checked in)" in out
    assert "`unresolved` region is stale" in err
    status, *_ = gate(tmp_path, CLOB, CLOB, "--write", "--strict")
    assert status == 0
    assert (tmp_path / "docs" / "rename-manifest.md").read_text() == manifest_text()
    capsys.readouterr()
    assert gate(tmp_path, CLOB, CLOB, "--strict")[0] == 0
    assert capsys.readouterr().out == ""


def test_write_leaves_the_prose_and_rows_as_they_are(tmp_path: Path) -> None:
    rows = ["`polyoxide_clob` → `polyoxide_polymarket::clob`"]
    write_manifest(tmp_path, paths=rows)
    gate(tmp_path, CLOB, MOVED, "--write")
    written = (tmp_path / "docs" / "rename-manifest.md").read_text()
    assert written == manifest_text(paths=rows, unresolved=region(written, "unresolved"))
    assert region(written, "unresolved").count("points nowhere") == 2


def test_the_regions_name_each_finding_in_one_code_span() -> None:
    entry = api_removals.Entry("struct", "polyoxide_a::`Odd`", False, None, None, "no row")
    [line] = api_removals.render_regions([entry], [], None)["unresolved"].splitlines()
    assert line == "- `` polyoxide_a::`Odd` `` struct: no row"
    assert "changed-in-place" not in api_removals.render_regions([], [], None)


def test_changes_in_place_come_from_cargo_semver_checks(tmp_path: Path) -> None:
    """The path diff cannot see a changed signature, nor an item newly hidden, whose
    path stays; a removal is the walker's to find."""
    write_manifest(tmp_path)
    report = mixed_and_clean().replace("--- failure struct_marked_non_exhaustive",
                                       "--- failure struct_now_doc_hidden")
    status, *_ = gate(tmp_path, CLOB, CLOB, report=(100, report))
    assert status == 0
    assert region(regenerated(tmp_path), "changed-in-place") == (
        "- `polyoxide-rtds struct_now_doc_hidden: struct DisplayPoint (src/payload.rs)`\n"
        "- `polyoxide-sports inherent_method_must_use_added: method "
        "polyoxide_sports::SportsWsBuilder::new (src/supervised.rs)`\n"
        "- `polyoxide-sports inherent_method_must_use_added: method "
        "polyoxide_sports::supervised::SportsWsBuilder::new (src/supervised.rs)`\n")


def test_without_cargo_semver_checks_the_changed_region_is_kept(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    text = write_manifest(tmp_path, changed="- `polyoxide-a some_lint: x`\n")
    status, _, stream, _ = gate(tmp_path, CLOB, CLOB, "--no-semver-checks", "--strict")
    assert status == 0
    assert not stream.called("cargo", "semver-checks")
    assert regenerated(tmp_path) == text
    assert "changed-in-place is kept" in capsys.readouterr().err


def test_the_manifest_must_name_the_same_baseline(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, baseline="v0.38.1")
    status, _, stream, snapshots = gate(tmp_path, CLOB, CLOB)
    assert status == 2
    assert "maps paths from v0.38.1, not v0.39.0" in capsys.readouterr().err
    assert snapshots.calls == [] and stream.calls == []


def test_an_unreadable_rustdoc_json_cannot_decide(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path)
    status, *_ = gate(tmp_path, PublicApiError("x.json is rustdoc JSON format 62"), CLOB)
    assert status == 2
    assert "::error::x.json is rustdoc JSON format 62" in capsys.readouterr().err


def test_the_gate_imports_the_manifests_doc_hidden_paths(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, hidden=["`polyoxide_perps::ws::IncomingForTests` `ws`",
                                     "`polyoxide_clob::DynSigner`",
                                     "`polyoxide_rtds::old` **Removed** by Story 5.5"])
    listed = snap("enum polyoxide_perps::ws::IncomingForTests", "type_alias polyoxide_clob::DynSigner",
                  hidden={"polyoxide_perps::ws::IncomingForTests"})
    status, _, stream, _ = gate(tmp_path, listed, listed,
                                compiled=fails_for("polyoxide-clob", ""))
    assert status == 1
    assert [Path(c[c.index("--manifest-path") + 1]).parent.name
            for c in stream.called("cargo", "check")] == ["polyoxide-perps+ws", "polyoxide-clob"]
    assert "The compile test of polyoxide-clob with default features failed" in (
        capsys.readouterr().err)


def test_a_gone_crate_in_the_manifests_hidden_paths_names_the_manifest(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, hidden=["`polyoxide_gone::fixtures` `test-server`"])
    listed = snap("module polyoxide_gone::fixtures", hidden={"polyoxide_gone::fixtures"})
    assert gate(tmp_path, listed, snap())[0] == 1
    assert "::error::docs/rename-manifest.md lists paths in crates" in capsys.readouterr().err


def sports_json(hidden_attr) -> dict:
    """rustdoc JSON for `polyoxide_sports` with a `#[doc(hidden)]` fixtures module, its
    attribute spelt as `hidden_attr`, and an unhidden type alias."""
    def item(id_: int, name: str, inner, attrs=()) -> dict:
        return {"id": id_, "crate_id": 0, "name": name, "visibility": "public",
                "attrs": list(attrs), "inner": inner}
    index = {
        "0": item(0, "polyoxide_sports", {"module": {"is_crate": True, "items": [1, 3]}}),
        "1": item(1, "fixtures", {"module": {"is_crate": False, "items": [2]}}, [hidden_attr]),
        "2": item(2, "SOCCER", {"constant": {}}),
        "3": item(3, "Socket", {"type_alias": {"type": {"primitive": "u8"}}}),
    }
    return {"root": 0, "index": index, "paths": {}, "external_crates": {}, "format_version": 57}


def sports_listing(tmp_path: Path, hidden_attr) -> Snapshot:
    directory = tmp_path / "json"
    directory.mkdir(exist_ok=True)
    (directory / "polyoxide_sports.json").write_text(json.dumps(sports_json(hidden_attr)))
    return Snapshot(api_removals.public_api.walk(directory, ["polyoxide_sports"]), frozenset())


SPORTS_HIDDEN = ["`polyoxide_sports::fixtures::SOCCER` `test-server`",
                 "`polyoxide_sports::Socket`",
                 "`polyoxide_sports::gone` `test-server` **Removed** by Story 5.5"]


def test_the_listed_hidden_paths_are_flagged_hidden_by_the_walker(tmp_path: Path) -> None:
    write_manifest(tmp_path, hidden=SPORTS_HIDDEN)
    listed = sports_listing(tmp_path, {"other": "#[doc(hidden)]"})
    assert listed.paths[("constant", "polyoxide_sports::fixtures::SOCCER")] is True
    assert gate(tmp_path, listed, listed)[0] == 0


@pytest.mark.parametrize("attr", [
    {"doc_hidden": {}},
    {"other": "#[attr = Doc(Hidden)]"},
    {"doc": {"hidden": True}},
])
def test_an_attribute_the_walker_cannot_read_cannot_decide(
        tmp_path: Path, attr, capsys: pytest.CaptureFixture[str]) -> None:
    """A rustdoc format that spells `#[doc(hidden)]` another way would drop every
    hidden path from the diff unseen; the listed paths catch it."""
    write_manifest(tmp_path, hidden=SPORTS_HIDDEN)
    listed = sports_listing(tmp_path, attr)
    status, _, stream, _ = gate(tmp_path, listed, listed)
    assert status == 2
    err = capsys.readouterr().err
    assert ("lists polyoxide_sports::fixtures::SOCCER, and the rustdoc JSON flags none of "
            "them hidden") in err
    assert "no longer recognises `#[doc(hidden)]` in this rustdoc format" in err
    assert "Socket" not in err and "gone" not in err
    assert not stream.calls


def test_a_listed_path_a_story_un_hid_is_named_and_the_format_not_blamed(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, hidden=SPORTS_HIDDEN + ["`polyoxide_sports::open`"])
    listed = sports_listing(tmp_path, {"other": "#[doc(hidden)]"})
    listed.paths[("function", "polyoxide_sports::open")] = False
    assert gate(tmp_path, listed, listed)[0] == 2
    err = capsys.readouterr().err
    assert "lists polyoxide_sports::open, which are no longer doc-hidden" in err
    assert "update that section to the paths consumers import now" in err
    assert "rustdoc format" not in err


def test_a_listed_path_neither_listing_has_cannot_decide(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_manifest(tmp_path, hidden=["`polyoxide_sports::fixtures::CRICKET` `test-server`"])
    listed = sports_listing(tmp_path, {"other": "#[doc(hidden)]"})
    assert gate(tmp_path, listed, listed)[0] == 2
    err = capsys.readouterr().err
    assert "lists polyoxide_sports::fixtures::CRICKET, which neither listing has" in err
    assert "rustdoc format" not in err


def test_a_listed_path_the_tree_no_longer_has_is_read_from_the_baseline(tmp_path: Path) -> None:
    """A path that moved before its entry did is still flagged at the baseline; the
    compile test is what fails it."""
    write_manifest(tmp_path, hidden=SPORTS_HIDDEN)
    listed = sports_listing(tmp_path, {"other": "#[doc(hidden)]"})
    head = Snapshot({("type_alias", "polyoxide_sports::Socket"): False}, frozenset())
    assert gate(tmp_path, listed, head)[0] == 0


@pytest.mark.parametrize(("stage", "group"), [("S2", "polyoxide-relay"), ("S1", "polyoxide-clob")])
def test_release_imports_the_manifests_paths_while_the_stage_is_s2(
        tmp_path: Path, stage: str, group: str) -> None:
    write_stage(tmp_path, stage)
    write_listing(tmp_path, hidden=["`polyoxide_clob::DynSigner`"])
    write_manifest(tmp_path, hidden=["`polyoxide_relay::DynSigner`"])
    run, stream = runners(tmp_path, (0, CLEAN), rev="v0.38.0")
    assert release(tmp_path, run, stream) == 0
    [build] = stream.called("cargo", "check")
    assert Path(build[build.index("--manifest-path") + 1]).parent.name == group


# --- the listing for the stage -------------------------------------------------


def built_groups(stream: Stub) -> list[str]:
    """The scratch crates the compile test checked, by their directories."""
    return [Path(c[c.index("--manifest-path") + 1]).parent.name
            for c in stream.called("cargo", "check")]


@pytest.mark.parametrize(("stage", "other", "baseline"), [
    ("S1", "S3", "v0.38.1"),
    ("S3", "S1", "v0.40.0"),
])
def test_check_and_release_read_the_listing_for_the_stage(
        tmp_path: Path, stage: str, other: str, baseline: str) -> None:
    """The commit after the S2 release sets the stage to S3, and from it `check` and
    `release` read docs/s3-removals.md: its baseline, its removals and its doc-hidden
    paths. The other stage's listing is there and differs in each, and neither
    command reads it."""
    write_stage(tmp_path, stage)
    write_listing(tmp_path, removed=REMOVAL_KEYS, hidden=["`polyoxide_relay::DynSigner`"],
                  baseline=baseline, listing=api_removals.LISTINGS[stage])
    write_listing(tmp_path, hidden=["`polyoxide_clob::DynSigner`"], baseline="v0.0.1",
                  listing=api_removals.LISTINGS[other])
    run, stream = runners(tmp_path, (100, mixed_and_clean()), rev=baseline)
    assert api_removals.main(["check", "--baseline", baseline], tmp_path, run, stream) == 0
    assert built_groups(stream) == ["polyoxide-relay"]
    run, stream = runners(tmp_path, (0, CLEAN), rev=baseline)
    assert release(tmp_path, run, stream, baseline) == 0
    assert built_groups(stream) == ["polyoxide-relay"]


def test_check_against_the_s3_listing_names_it_on_an_unlisted_removal(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    write_stage(tmp_path, "S3")
    write_listing(tmp_path, baseline="v0.40.0", listing=api_removals.LISTINGS["S3"])
    run, stream = runners(tmp_path, (100, mixed_and_clean()), rev="v0.40.0")
    assert api_removals.main(["check", "--baseline", "v0.40.0"], tmp_path, run, stream) == 1
    assert "that docs/s3-removals.md does not list" in capsys.readouterr().err


def test_check_refuses_stage_s2_whose_gate_is_the_manifest(
        tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """S2 has no removal listing. Reading S1's against the S2 tree would gate on
    nothing that matters, so `check` refuses before running anything."""
    write_stage(tmp_path, "S2")
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, CLEAN))
    assert check(tmp_path, run, stream) == 2
    err = capsys.readouterr().err
    assert "stage S2 has no removal listing: its gate is `manifest`" in err
    assert "docs/s3-removals.md" in err
    assert stream.calls == []


@pytest.mark.parametrize("command", ["check", "release"])
def test_a_stage_without_a_listing_cannot_decide(
        tmp_path: Path, capsys: pytest.CaptureFixture[str], command: str) -> None:
    write_stage(tmp_path, "S4")
    write_listing(tmp_path)
    run, stream = runners(tmp_path, (0, CLEAN))
    assert api_removals.main([command, "--baseline", "v0.38.1"], tmp_path, run, stream) == 2
    assert "stage S4 has no removal listing" in capsys.readouterr().err


# --- the real tree -----------------------------------------------------------


def test_the_real_s3_listing_starts_at_the_s2_release_and_removes_nothing() -> None:
    listing = api_removals.read_listing(REPO, api_removals.LISTINGS["S3"])
    assert listing.baseline == "v0.40.0"
    assert listing.removed == frozenset()
    assert listing.hidden, "the compile test would import nothing"


def test_every_path_the_s3_listing_imports_names_a_workspace_crate() -> None:
    publish_order = _load("publish_order")
    metadata = publish_order.workspace_metadata()
    libraries = {p["name"].replace("-", "_") for p in publish_order.members(metadata)}
    for hidden in api_removals.read_listing(REPO, api_removals.LISTINGS["S3"]).hidden:
        assert hidden.crate in libraries, hidden.path


def test_the_s3_listing_imports_the_manifests_live_paths_while_the_stage_is_s2() -> None:
    """The S3 gate starts from the paths consumers import at the S2 release, so
    neither the removals job nor the release loses one at the switch. Once the stage
    moves on, S3's stories edit the listing and the frozen manifest stays as it is."""
    if api_removals.workspace_stage(REPO) != "S2":
        pytest.skip("the S3 listing follows its own stories once the stage is S3")
    manifest = {(h.path, h.features) for h in api_removals.read_manifest(REPO).hidden
                if not h.removed}
    listing = api_removals.read_listing(REPO, api_removals.LISTINGS["S3"])
    assert {(h.path, h.features) for h in listing.hidden if not h.removed} == manifest




def test_the_real_s1_listing_is_final_and_still_reads() -> None:
    """Nothing gates on it after S1, but `check` and `release` read it again should
    the stage ever be S1."""
    listing = api_removals.read_listing(REPO)
    assert listing.baseline == "v0.38.1"
    assert listing.removed and listing.hidden


def test_the_real_manifest_matches_the_removals_job() -> None:
    manifest = api_removals.read_manifest(REPO)
    ci = yaml.safe_load((REPO / ".github/workflows/ci.yml").read_text())
    assert ci["env"]["S2_BASELINE"] == manifest.baseline == "v0.39.0"
    assert manifest.hidden, "the compile test would import nothing"
    assert set(manifest.regions) == set(api_removals.REGIONS)


def test_every_path_the_manifest_imports_names_a_workspace_crate() -> None:
    publish_order = _load("publish_order")
    metadata = publish_order.workspace_metadata()
    libraries = {p["name"].replace("-", "_") for p in publish_order.members(metadata)}
    for hidden in api_removals.read_manifest(REPO).hidden:
        assert hidden.crate in libraries, hidden.path


def test_the_manifest_imports_every_live_s1_path_until_a_row_moves_it() -> None:
    """The S1 listing's live paths seed the manifest's section, so neither the
    removals job nor the release loses a path at the switch. A path a row moves is
    imported at its new path; a path a reason row removes stays listed, marked
    **Removed**."""
    manifest = api_removals.read_manifest(REPO)
    live = {hidden.path for hidden in manifest.hidden if not hidden.removed}
    removed = {hidden.path for hidden in manifest.hidden if hidden.removed}
    for hidden in api_removals.read_listing(REPO).hidden:
        if hidden.removed:
            continue
        row = api_removals.longest_row(manifest.rows, hidden.path)
        if row is None:
            assert hidden.path in live, hidden.path
        elif row.new is not None:
            assert row.maps(hidden.path) in live, (hidden.path, row)
        else:
            assert hidden.path in removed, (hidden.path, row)


def test_the_frozen_rename_map_follows_the_manifests_rows() -> None:
    """docs/rename-map.tsv was written once by `manifest --expand` from the S2
    release's tree. Each line is an entry its manifest row resolves, to the path the
    row maps it to or to the row's reason, and every row resolves at least one."""
    manifest = api_removals.read_manifest(REPO)
    lines = (REPO / "docs" / "rename-map.tsv").read_text(encoding="utf-8").splitlines()
    assert len(lines) > 10_000, "the map holds too few entries to be the S2 map"
    used = set()
    for line in lines:
        old, target, kind, status = line.split("\t")
        assert status in ("resolved", "resolved, doc-hidden"), line
        rows, separator = ((manifest.features, "/") if kind == "feature"
                           else (manifest.rows, "::"))
        row = api_removals.longest_row(rows, old, separator)
        assert row is not None, line
        used.add(row)
        expected = row.maps(old) if row.new is not None else row.reason
        assert target == api_removals._tsv_field(expected), line
    assert used == set(manifest.rows) | set(manifest.features)


def test_the_real_workspace_is_in_stage_s2() -> None:
    """While it is, the release's compile test reads the manifest."""
    assert api_removals.workspace_stage(REPO) == "S2"


def _pins(workflow: str, job: str) -> tuple[str, str]:
    steps = yaml.safe_load((REPO / ".github/workflows" / workflow).read_text())["jobs"][job]["steps"]
    toolchain = next(s["uses"] for s in steps if s.get("uses", "").startswith("dtolnay/"))
    tool = next(s["with"]["tool"] for s in steps
                if s.get("uses", "").startswith("taiki-e/install-action"))
    return toolchain.split("@")[1], tool.split("@")[1]


def test_the_captured_fixtures_were_made_with_the_pinned_versions() -> None:
    """Recapture the fixtures when the pins move: a new tool version may print its
    report differently, and the parser is only proved against this one."""
    provenance = (FIXTURES / "PROVENANCE.md").read_text()
    tool = re.search(r"^- Tool: cargo-semver-checks (\S+)$", provenance, re.M)[1]
    rust = re.search(r"^- Toolchain: Rust (\S+)$", provenance, re.M)[1]
    assert _pins("ci.yml", "removals") == _pins("release.yml", "semver") == (rust, tool)
    assert f"cargo-semver-checks/tree/v{tool}/" in captured("mixed.txt")
