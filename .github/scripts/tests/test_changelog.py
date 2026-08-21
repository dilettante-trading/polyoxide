"""Consistency checks between the workspace version and CHANGELOG.md.

Releasing is two independent manual steps that must agree: bump
`[workspace.package] version` in Cargo.toml, and write the matching section
into CHANGELOG.md. Nothing verified the second one happened.

It has already failed twice. 0.27.0 shipped with no entry at all -- the bump
rode along inside `chore(specs): sync the data spec and release 0.27.0` rather
than a dedicated release commit, so the step that regenerates CHANGELOG.md was
never run, and the file jumped 0.28.0 -> 0.26.1 with nothing to signal it. The
gap sat undetected until someone happened to read the file.

CI cannot catch this on its own. `release.yml` runs git-cliff with
`--latest --strip header` purely to compose the GitHub release *body*; it never
writes the file back, so CHANGELOG.md is only ever updated by hand.

These tests read Cargo.toml and CHANGELOG.md and nothing else -- no git, no
network. That matters: `actions/checkout` clones shallow and tagless by
default, and CI runs on every branch, so any invariant phrased against tags or
against "commits since the last release" would be both unavailable and wrong
(it would fail on every in-flight branch). What is asserted here holds on every
commit of every branch.

Deliberately NOT covered: a commit that lands *after* the release commit but
*before* CI cuts the tag is released with no changelog entry, and this file
cannot see it -- the version and its section both exist, so every assertion
below passes. Three commits of the 0.28.0 cycle were missed exactly that way.
Guarding that needs the release commit to be the last commit before the tag,
which is a property of the process, not of these two files.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]

# `## [0.28.0] - 2026-08-19`. The date is matched, not merely tolerated: an
# undated heading is how a half-finished release announces itself.
SECTION_RE = re.compile(
    r"^## \[(?P<version>[^\]]+)\] - (?P<date>\d{4}-\d{2}-\d{2})$", re.MULTILINE
)

# The pre-fork CLI had its own release series under a `cli-v` prefix. Those
# sections are frozen history and sort against a different origin, so they are
# held apart from the workspace series rather than interleaved with it.
LEGACY_PREFIX = "cli-v"

# `version = "0.28.0"` under `[workspace.package]`, and the same string repeated
# in each `[workspace.dependencies]` path pin.
WORKSPACE_VERSION_RE = re.compile(r'^version = "(?P<version>[^"]+)"$', re.MULTILINE)
PATH_PIN_RE = re.compile(
    r'^(?P<crate>polyoxide-[a-z]+) = \{ path = "[^"]+", version = "(?P<version>[^"]+)" \}$',
    re.MULTILINE,
)


def _changelog() -> str:
    return (REPO / "CHANGELOG.md").read_text()


def _cargo() -> str:
    return (REPO / "Cargo.toml").read_text()


def sections() -> list[tuple[str, str]]:
    """Every `## [version] - date` heading, in the order the file lists them."""
    return [
        (m.group("version"), m.group("date"))
        for line in _changelog().splitlines()
        if (m := SECTION_RE.match(line))
    ]


def workspace_version() -> str:
    """The version every crate inherits via `version.workspace = true`.

    `[workspace.package]` is the first table in Cargo.toml carrying a bare
    `version` key, so the first match is the workspace version. The pins under
    `[workspace.dependencies]` use a different shape and cannot collide.
    """
    match = WORKSPACE_VERSION_RE.search(_cargo())
    assert match is not None, (
        "No `version = \"...\"` line found in Cargo.toml. The workspace manifest "
        "was restructured; update WORKSPACE_VERSION_RE in this file to match."
    )
    return match.group("version")


def _order_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def test_workspace_version_has_a_changelog_section() -> None:
    """The version Cargo.toml claims must be a released, dated section.

    This is the assertion that 0.27.0 would have failed. release.yml greps this
    same version out of Cargo.toml and cuts a tag from it, so a version with no
    section is a release with no notes.
    """
    version = workspace_version()
    documented = {found for found, _ in sections()}
    assert version in documented, (
        f"Cargo.toml is at {version} but CHANGELOG.md has no `## [{version}] - <date>` "
        f"section. Add one before the bump reaches main -- release.yml cuts tag "
        f"v{version} from this version, and nothing writes CHANGELOG.md back for you. "
        f"Generate the entries with `git-cliff --unreleased --tag v{version}`."
    )


def test_workspace_version_is_the_newest_section() -> None:
    """The version under construction heads the file.

    A bump that lands with its section buried mid-file means the entries were
    written into the wrong place, which reads as a complete changelog while
    describing the wrong release.
    """
    version = workspace_version()
    found = sections()
    assert found, "CHANGELOG.md contains no version sections at all."
    newest = found[0][0]
    assert newest == version, (
        f"CHANGELOG.md leads with `{newest}` but Cargo.toml is at {version}. "
        f"The newest section must be the version being released."
    )


def test_workspace_sections_are_strictly_descending() -> None:
    """No duplicates, no out-of-order entries, no silent gaps.

    The 0.27.0 miss left the file reading 0.28.0 -> 0.26.1. Ordering is what
    makes a hole like that visible: a backfilled section has exactly one
    correct slot, and anything else fails here.
    """
    versions = [v for v, _ in sections() if not v.startswith(LEGACY_PREFIX)]
    ordered = sorted(versions, key=_order_key, reverse=True)
    assert versions == ordered, (
        "CHANGELOG.md sections are not in strictly descending version order. "
        f"Found {versions[:8]}..., expected to start {ordered[:8]}.... "
        "A backfilled entry belongs in its chronological slot."
    )
    assert len(versions) == len(set(versions)), (
        "CHANGELOG.md lists the same version twice: "
        f"{sorted({v for v in versions if versions.count(v) > 1})}."
    )


def test_legacy_cli_sections_follow_the_workspace_series() -> None:
    """Guard the split above: `cli-v*` is frozen history and stays at the tail.

    Without this, a workspace section accidentally written below the legacy
    block would be skipped by the ordering check rather than caught by it.
    """
    kinds = [v.startswith(LEGACY_PREFIX) for v, _ in sections()]
    assert kinds == sorted(kinds), (
        "A workspace version section appears below the legacy `cli-v*` block. "
        "Workspace releases go above them, newest first."
    )


def test_workspace_dependency_pins_match_the_workspace_version() -> None:
    """The bump has five other places to land, and publishing needs all of them.

    Each in-workspace dependency is pinned `{ path = ..., version = "X.Y.Z" }`.
    cargo publish resolves the `version`, not the `path`, so a pin left behind
    at the previous release makes the crate ask crates.io for a version that
    does not exist yet -- and it fails partway through an ordered,
    already-partly-published release rather than before it starts.
    """
    version = workspace_version()
    pins = PATH_PIN_RE.findall(_cargo())
    assert pins, (
        "No `[workspace.dependencies]` path pins found in Cargo.toml. The manifest "
        "was restructured; update PATH_PIN_RE in this file to match."
    )
    stale = {crate: pinned for crate, pinned in pins if pinned != version}
    assert not stale, (
        f"Cargo.toml is at {version} but these [workspace.dependencies] pins were "
        f"left behind: {stale}. Every path pin must move with the workspace version."
    )


@pytest.mark.parametrize("regex", [SECTION_RE, WORKSPACE_VERSION_RE, PATH_PIN_RE])
def test_parsers_still_match_something(regex: re.Pattern[str]) -> None:
    """Guard the guards: a reformat must fail loudly, not pass vacuously.

    Every assertion above is built on one of these three patterns. If a file is
    reformatted so a pattern stops matching, the checks would quietly degrade
    into assertions about an empty list. This separates "the invariant broke"
    from "the parser stopped seeing the file".
    """
    haystack = _changelog() if regex is SECTION_RE else _cargo()
    # Bound to a name first: asserting on the `search` directly makes pytest dump
    # the entire file into the failure, burying the message.
    matched = regex.search(haystack) is not None
    assert matched, (
        f"{regex.pattern!r} no longer matches anything. The file it reads was "
        f"reformatted; update the pattern in this file."
    )
