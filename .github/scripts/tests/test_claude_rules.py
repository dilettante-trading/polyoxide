"""The rules files under `.claude/rules/` load only where their globs point.

CLAUDE.md holds the rules that cut across crates. Each topic's rules live in
`.claude/rules/<topic>.md`, whose YAML frontmatter names the `paths:` globs that make Claude
Code load it, when Claude reads or edits a matching file. A rules file with no `paths`, or
whose frontmatter does not parse, loads into every session instead, which silently undoes the
split. These tests fail on each way that can happen: a file with no frontmatter, frontmatter
that does not parse or names no globs, a glob or one of its `{a,b}` alternatives that matches no
file in the tree (it rots unseen when files move), and a rules file CLAUDE.md's table does not
list, or a listed one that is gone.
"""

from __future__ import annotations

import re
import subprocess
from functools import cache
from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[3]
RULES = REPO / ".claude" / "rules"
FRONTMATTER = re.compile(r"\A---\n(?P<yaml>.*?)\n---\n", re.DOTALL)
# A row of CLAUDE.md's table of rules files: "| `<topic>.md` | <what it covers> |".
TABLE_ROW = re.compile(r"^\| `(?P<name>[a-z0-9-]+\.md)` \|", re.MULTILINE)


@cache
def tree() -> tuple[str, ...]:
    """Every file in the working tree that git tracks or would track."""
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=REPO, check=True, capture_output=True, text=True)
    return tuple(out.stdout.splitlines())


def glob_pattern(glob: str) -> str:
    """The regex for a glob matched against a path from the repository root: `**/` is any
    number of directories, `**` is anything, `*` and `?` stay within one path segment, and
    `{a,b}` is either alternative, each itself a glob."""
    out, i = "", 0
    while i < len(glob):
        if glob.startswith("**/", i):
            out, i = out + "(?:.*/)?", i + 3
        elif glob.startswith("**", i):
            out, i = out + ".*", i + 2
        elif glob[i] == "*":
            out, i = out + "[^/]*", i + 1
        elif glob[i] == "?":
            out, i = out + "[^/]", i + 1
        elif glob[i] == "{":
            end = glob.index("}", i)
            alternatives = glob[i + 1:end].split(",")
            out += "(?:" + "|".join(glob_pattern(alt) for alt in alternatives) + ")"
            i = end + 1
        else:
            out, i = out + re.escape(glob[i]), i + 1
    return out


def alternatives(glob: str) -> list[str]:
    """`glob` with each `{a,b}` group expanded, so every alternative can be checked alone."""
    start = glob.find("{")
    if start < 0:
        return [glob]
    end = glob.index("}", start)
    head, tail = glob[:start], glob[end + 1:]
    return [expanded for alt in glob[start + 1:end].split(",")
            for expanded in alternatives(head + alt + tail)]


def matches(glob: str) -> list[str]:
    pattern = re.compile(glob_pattern(glob))
    return [path for path in tree() if pattern.fullmatch(path)]


def rules_files() -> list[Path]:
    return sorted(RULES.glob("*.md"))


@pytest.mark.parametrize(("glob", "path", "expected"), [
    ("polyoxide-core/**", "polyoxide-core/src/send.rs", True),
    ("**/tests/live_*.rs", "polyoxide-cli/tests/live_api.rs", True),
    ("**/tests/live_*.rs", "tests/live_api.rs", True),
    ("**/tests/live_*.rs", "polyoxide-cli/tests/common/live_api.rs", False),
    ("polyoxide-*/src/**/client.rs", "polyoxide-polymarket/src/client.rs", True),
    ("polyoxide-*/src/**/client.rs", "polyoxide-polymarket/src/gamma/client.rs", True),
    ("scripts/{publish_order.py,finish_release.sh}", "scripts/finish_release.sh", True),
    ("examples/{v2_soak/**,x.rs}", "examples/v2_soak/main.rs", True),
    ("*.md", "docs/x.md", False),
])
def test_a_glob_matches_what_it_names(glob: str, path: str, expected: bool) -> None:
    assert (re.fullmatch(glob_pattern(glob), path) is not None) is expected


def test_each_brace_group_expands_into_its_alternatives() -> None:
    assert alternatives("a/{b,c}/{d,e}.rs") == ["a/b/d.rs", "a/b/e.rs", "a/c/d.rs", "a/c/e.rs"]
    assert alternatives("a/**/b.rs") == ["a/**/b.rs"]


def test_there_are_rules_files() -> None:
    assert rules_files(), f"no rules files under {RULES.relative_to(REPO)}"


@pytest.mark.parametrize("rule", rules_files(), ids=lambda rule: rule.name)
def test_each_rules_file_loads_only_for_files_its_globs_match(rule: Path) -> None:
    found = FRONTMATTER.match(rule.read_text())
    assert found, f"{rule.name} has no `---` frontmatter, so it loads into every session"
    meta = yaml.safe_load(found["yaml"])
    globs = meta.get("paths") if isinstance(meta, dict) else None
    assert isinstance(globs, list) and globs and all(isinstance(g, str) and g for g in globs), (
        f"{rule.name}: `paths` must be a non-empty list of glob strings, or the file loads into "
        f"every session; the frontmatter reads {meta!r}")
    dead = [alt for glob in globs for alt in alternatives(glob) if not matches(alt)]
    assert not dead, f"{rule.name}: these globs, or alternatives of one, match no file: {dead}"


def test_claude_md_lists_every_rules_file_once() -> None:
    listed = TABLE_ROW.findall((REPO / "CLAUDE.md").read_text())
    twice = sorted({name for name in listed if listed.count(name) > 1})
    assert not twice, f"CLAUDE.md's table lists these rules files more than once: {twice}"
    on_disk = {rule.name for rule in rules_files()}
    unlisted = sorted(on_disk - set(listed))
    assert not unlisted, f"rules files CLAUDE.md's table does not list: {unlisted}"
    missing = sorted(set(listed) - on_disk)
    assert not missing, f"CLAUDE.md's table lists rules files that do not exist: {missing}"
