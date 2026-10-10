"""Tests for scripts/finish_release.sh, the one resumable publish loop.

The script is copied into a temporary repository and run with stub `python3`,
`cargo`, `sleep` and `git` first on PATH. The `python3` stub answers each
`publish_order.py list` call from a script the test writes, and every stub logs
its arguments, so a test sees what would have been published, and when, without
cargo, crates.io or a 30-second sleep.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
SCRIPT = REPO / "scripts" / "finish_release.sh"

LIST = "scripts/publish_order.py list --max-new-names 5"
RECOUNT = "scripts/publish_order.py list"
HEAD = "a" * 40

STUBS = {
    # `ci-passed` exits ci.code. Otherwise the n-th call prints list.<n>.out and
    # exits list.<n>.code; an unscripted call exits 97, which reads as a failed
    # list and shows up in the call count.
    "python3": """\
if [ "$2" = "ci-passed" ]; then
  printf '%s\\n' "$*" >> "$STUBS/ci.calls"
  exit "$(cat "$STUBS/ci.code")"
fi
printf '%s\\n' "$*" >> "$STUBS/python3.calls"
n=$(wc -l < "$STUBS/python3.calls" | tr -d ' ')
if [ ! -f "$STUBS/list.$n.code" ]; then
  echo "unscripted python3 call $n: $*" >&2
  exit 97
fi
cat "$STUBS/list.$n.out"
exit "$(cat "$STUBS/list.$n.code")"
""",
    # The n-th call exits cargo.<n>.code, or 0.
    "cargo": """\
printf '%s\\n' "$*" >> "$STUBS/cargo.calls"
n=$(wc -l < "$STUBS/cargo.calls" | tr -d ' ')
if [ -f "$STUBS/cargo.$n.code" ]; then
  exit "$(cat "$STUBS/cargo.$n.code")"
fi
""",
    "sleep": """\
printf '%s\\n' "$*" >> "$STUBS/sleep.calls"
""",
    # `status` prints git.status; `merge-base` exits git.ancestor; `rev-parse`
    # prints HEAD.
    "git": f"""\
printf '%s\\n' "$*" >> "$STUBS/git.calls"
case "$1" in
  status) cat "$STUBS/git.status" ;;
  merge-base) exit "$(cat "$STUBS/git.ancestor")" ;;
  rev-parse) echo {HEAD} ;;
esac
""",
}


class Harness:
    def __init__(self, tmp_path: Path) -> None:
        self.root = (tmp_path / "repo").resolve()
        (self.root / "scripts").mkdir(parents=True)
        shutil.copy(SCRIPT, self.root / "scripts" / "finish_release.sh")
        self.stubs = tmp_path / "stubs"
        self.stubs.mkdir()
        self.bin = tmp_path / "bin"
        self.bin.mkdir()
        for name, body in STUBS.items():
            path = self.bin / name
            path.write_text("#!/usr/bin/env bash\n" + body)
            path.chmod(0o755)
        self.elsewhere = tmp_path

    def crate(self, name: str) -> str:
        return f"{name} 1.0.0 {self.root}/{name}/Cargo.toml"

    def tombstone(self, name: str) -> str:
        return f"{name} 0.9.9 {self.root}/tombstones/{name}/Cargo.toml"

    def lists(self, *answers: tuple[int, list[str]]) -> None:
        """What each `publish_order.py list` call prints and exits, in order."""
        for n, (code, lines) in enumerate(answers, start=1):
            (self.stubs / f"list.{n}.out").write_text("".join(f"{line}\n" for line in lines))
            (self.stubs / f"list.{n}.code").write_text(str(code))

    def cargo_exits(self, *codes: int) -> None:
        for n, code in enumerate(codes, start=1):
            (self.stubs / f"cargo.{n}.code").write_text(str(code))

    def run(self, *, by_hand: bool = False, git_status: str = "", on_main: bool = True,
            ci_passed: bool = True) -> subprocess.CompletedProcess:
        (self.stubs / "git.status").write_text(git_status)
        (self.stubs / "git.ancestor").write_text("0" if on_main else "1")
        (self.stubs / "ci.code").write_text("0" if ci_passed else "1")
        env = {k: v for k, v in os.environ.items() if k != "GITHUB_ACTIONS"}
        if not by_hand:
            env["GITHUB_ACTIONS"] = "true"
        env["PATH"] = f"{self.bin}{os.pathsep}{env['PATH']}"
        env["STUBS"] = str(self.stubs)
        return subprocess.run(
            ["bash", str(self.root / "scripts" / "finish_release.sh")],
            cwd=self.elsewhere, env=env, capture_output=True, text=True, timeout=60,
        )

    def calls(self, stub: str) -> list[str]:
        path = self.stubs / f"{stub}.calls"
        return path.read_text().splitlines() if path.exists() else []


@pytest.fixture
def harness(tmp_path: Path) -> Harness:
    return Harness(tmp_path)


def test_an_empty_list_publishes_nothing(harness: Harness) -> None:
    harness.lists((0, []))
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("python3") == [LIST]
    assert harness.calls("cargo") == []
    assert harness.calls("sleep") == []


def test_too_many_new_names_stops_at_once(harness: Harness) -> None:
    """Exit 3 cannot change on a retry, so it is final and passed through."""
    harness.lists((3, []))
    result = harness.run()
    assert result.returncode == 3
    assert harness.calls("python3") == [LIST]
    assert harness.calls("cargo") == []
    assert harness.calls("sleep") == []


def test_an_unpublishable_workspace_stops_at_once(harness: Harness) -> None:
    """Exit 4 is a cycle, an unpublishable dependency or a bad manifest: not transient."""
    harness.lists((4, []))
    result = harness.run()
    assert result.returncode == 4
    assert "not transient" in result.stdout
    assert harness.calls("python3") == [LIST]
    assert harness.calls("cargo") == []
    assert harness.calls("sleep") == []


def test_a_rerun_after_a_partial_publish_publishes_only_the_remainder(
        harness: Harness) -> None:
    """The first attempt uploads `a` and fails; the second lists only `b` and `c`."""
    a, b, c = harness.crate("a"), harness.crate("b"), harness.crate("c")
    harness.lists((0, [a, b, c]), (0, [b, c]), (0, []))
    harness.cargo_exits(101, 0)
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("cargo") == [
        "publish --no-verify -p a -p b -p c",
        "publish --no-verify -p b -p c",
    ]
    assert harness.calls("sleep") == ["30"]


def test_pairs_left_after_three_attempts_fail(harness: Harness) -> None:
    a = harness.crate("a")
    harness.lists((0, [a]), (0, [a]), (0, [a]), (0, [a]))
    harness.cargo_exits(101, 101, 101)
    result = harness.run()
    assert result.returncode == 1
    assert "Still unpublished" in result.stdout
    assert a in result.stdout
    assert len(harness.calls("cargo")) == 3
    assert harness.calls("python3") == [LIST, LIST, LIST, RECOUNT]
    assert harness.calls("sleep") == ["30", "30", "30"]


def test_one_cargo_call_covers_the_workspace_and_tombstones_go_by_manifest(
        harness: Harness) -> None:
    old, older = harness.tombstone("old"), harness.tombstone("older")
    harness.lists((0, [harness.crate("core"), harness.crate("venue"), old, older]), (0, []))
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("cargo") == [
        "publish --no-verify -p core -p venue",
        f"publish --no-verify --manifest-path {harness.root}/tombstones/old/Cargo.toml",
        f"publish --no-verify --manifest-path {harness.root}/tombstones/older/Cargo.toml",
    ]


def test_tombstones_alone_make_no_workspace_call(harness: Harness) -> None:
    """An empty workspace array must expand to nothing under `set -u`."""
    harness.lists((0, [harness.tombstone("old")]), (0, []))
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("cargo") == [
        f"publish --no-verify --manifest-path {harness.root}/tombstones/old/Cargo.toml",
    ]


def test_a_failed_tombstone_upload_uses_an_attempt(harness: Harness) -> None:
    old = harness.tombstone("old")
    harness.lists((0, [harness.crate("core"), old]), (0, [old]), (0, []))
    harness.cargo_exits(0, 101, 0)
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("cargo")[1:] == [
        f"publish --no-verify --manifest-path {harness.root}/tombstones/old/Cargo.toml",
    ] * 2
    assert harness.calls("sleep") == ["30"]


@pytest.mark.parametrize("code", [1, 2])
def test_any_other_list_failure_uses_an_attempt(harness: Harness, code: int) -> None:
    """2 is argparse's, or python's for a missing script: neither is final."""
    harness.lists((code, []), (0, []))
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("python3") == [LIST, LIST]
    assert harness.calls("sleep") == ["30"]


def _three_attempts_ending_in_a_publish(harness: Harness, *recounts: tuple[int, list[str]]):
    a = harness.crate("a")
    harness.lists((0, [a]), (0, [a]), (0, [a]), *recounts)
    harness.cargo_exits(101, 101, 0)


def test_a_transient_recount_is_retried(harness: Harness) -> None:
    _three_attempts_ending_in_a_publish(harness, (1, []), (0, []))
    result = harness.run()
    assert result.returncode == 0, result.stderr
    assert harness.calls("python3") == [LIST, LIST, LIST, RECOUNT, RECOUNT]
    assert harness.calls("sleep") == ["30", "30", "30"]


def test_a_recount_that_never_answers_could_not_confirm(harness: Harness) -> None:
    """Reported apart from "still unpublished": the publish may well have worked."""
    _three_attempts_ending_in_a_publish(harness, (1, []), (1, []), (1, []))
    result = harness.run()
    assert result.returncode == 1
    assert "Could not confirm" in result.stdout
    assert "Still unpublished" not in result.stdout
    assert harness.calls("python3") == [LIST, LIST, LIST, RECOUNT, RECOUNT, RECOUNT]


def test_a_ci_run_skips_the_hand_run_checks(harness: Harness) -> None:
    harness.lists((0, []))
    result = harness.run(git_status=" M Cargo.toml\n", on_main=False, ci_passed=False)
    assert result.returncode == 0, result.stderr
    assert harness.calls("git") == []
    assert harness.calls("ci") == []


def test_a_hand_run_refuses_a_dirty_tree(harness: Harness) -> None:
    harness.lists((0, [harness.crate("a")]))
    result = harness.run(by_hand=True, git_status=" M Cargo.toml\n")
    assert result.returncode == 1
    assert "uncommitted changes" in result.stderr
    assert harness.calls("python3") == []
    assert harness.calls("cargo") == []


def test_a_hand_run_refuses_a_head_not_on_main(harness: Harness) -> None:
    harness.lists((0, [harness.crate("a")]))
    result = harness.run(by_hand=True, on_main=False)
    assert result.returncode == 1
    assert "not on origin/main" in result.stderr
    assert harness.calls("git")[1:] == [
        "fetch --quiet origin main",
        "merge-base --is-ancestor HEAD origin/main",
    ]
    assert harness.calls("python3") == []
    assert harness.calls("cargo") == []


def test_a_hand_run_refuses_a_commit_ci_has_not_passed(harness: Harness) -> None:
    """Every publish is --no-verify, which only CI having passed justifies."""
    harness.lists((0, [harness.crate("a")]))
    result = harness.run(by_hand=True, ci_passed=False)
    assert result.returncode == 1
    assert "CI has not passed on HEAD" in result.stderr
    assert harness.calls("ci") == [f"scripts/publish_order.py ci-passed {HEAD}"]
    assert harness.calls("python3") == []
    assert harness.calls("cargo") == []


def test_a_clean_hand_run_on_main_publishes(harness: Harness) -> None:
    harness.lists((0, [harness.crate("a")]), (0, []))
    result = harness.run(by_hand=True)
    assert result.returncode == 0, result.stderr
    assert harness.calls("git") == [
        "status --porcelain",
        "fetch --quiet origin main",
        "merge-base --is-ancestor HEAD origin/main",
        "rev-parse HEAD",
    ]
    assert harness.calls("ci") == [f"scripts/publish_order.py ci-passed {HEAD}"]
    assert harness.calls("cargo") == ["publish --no-verify -p a"]
