"""Unit tests for scripts/api_removals.py.

The report parser and the compile test's error reader run on output captured from
cargo-semver-checks 0.51.0 and cargo under `fixtures/semver-checks/` (see its
PROVENANCE.md). Everything that would run git, cargo or cargo-semver-checks runs a
stub instead, so nothing here builds a crate. The tests of the real tree read
docs/s1-removals.md, the workflows and `cargo metadata --offline`.
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


def write_listing(root: Path, removed: list[str] = (), hidden: list[str] = (),
                  baseline: str = "v0.38.1") -> None:
    lines = ["---", f"baseline: {baseline}", "---", "", "## Removed", ""]
    lines += [f"- `{key}` Story 9.9: use something else." for key in removed]
    lines += ["", "## Doc-hidden paths consumers import", ""]
    lines += [f"- {entry}" for entry in hidden]
    (root / "docs").mkdir(parents=True, exist_ok=True)
    (root / "docs" / "s1-removals.md").write_text("\n".join(lines) + "\n")


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


# --- the real tree -----------------------------------------------------------


def test_the_real_listing_matches_the_removals_job() -> None:
    listing = api_removals.read_listing(REPO)
    ci = yaml.safe_load((REPO / ".github/workflows/ci.yml").read_text())
    assert ci["jobs"]["removals"]["env"]["S1_BASELINE"] == listing.baseline
    assert listing.hidden, "the compile test would import nothing"


def test_every_listed_path_names_a_workspace_crate() -> None:
    publish_order = _load("publish_order")
    metadata = publish_order.workspace_metadata()
    libraries = {p["name"].replace("-", "_") for p in publish_order.members(metadata)}
    for hidden in api_removals.read_listing(REPO).hidden:
        assert hidden.crate in libraries, hidden.path


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
