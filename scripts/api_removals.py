#!/usr/bin/env python3
"""The removal gate: public items removed since a baseline, against a checked-in list.

cargo-semver-checks compares each workspace crate's public API with the same crate
at a baseline git revision. This script runs it and keys each item it reports as

    <crate> <lint id>: <item> (<file>)

where `<item>` is the tool's own "Failed in:" line without its location, and
`<file>` is the item's source file relative to its crate, without a line number.
The file tells same-named items apart (polyoxide-polymarket's `data` module has a
`ListTrades` in v1 and in v2), and leaving out the line keeps a key stable across
edits elsewhere in the file. A publishable crate that is in the baseline's workspace and not in this one
is keyed `<crate> crate_missing`. Three subcommands:

    manifest CI's removals job through stage S2. Lists every public path at the
             baseline and in the working tree from rustdoc JSON (public_api.py),
             and calls each baseline path, and each baseline `<crate>/<feature>`,
             that the tree does not have an entry. Resolves each entry through
             docs/rename-manifest.md's rows, by the row with the longest old path
             at a `::` (or, for features, `/`) boundary: a reason row resolves it,
             and a path row resolves it if the tree has the path it maps to under
             the same kind, and doc-hidden only if the old path was. A row
             covering no entry is stale. Regenerates the manifest's
             three generated regions (`unresolved`, `stale`, and
             `changed-in-place`, cargo-semver-checks' lints on paths that stay)
             into target/api-removals/rename-manifest.md, and over the manifest
             with `--write`, and prints each region's diff. Each finding is a
             warning, and `--strict`, which CI's removals job passes since
             Story 5.12, fails on an unresolved entry, a stale row or a stale
             region. `--expand FILE`
             writes every entry as `old<TAB>new or reason<TAB>kind<TAB>status`.
    check    The S1 removals job, which returns in S3. Runs cargo-semver-checks
             with `--release-type patch`, which makes every lint required, so
             removals are reported even across a 0.x minor bump. Exits 1 naming
             each removal that docs/s1-removals.md does not list under "Removed".
             Other lints are printed and allowed: S1 changes error and builder
             shapes on purpose.
    release  release.yml's semver job. Runs cargo-semver-checks against the
             previous release's tag and lets it infer the release type from the
             two versions, so a 0.x patch bump may break nothing. Exits 1 on any
             lint failure, and on a deleted crate unless the bump raises the 0.x
             minor (AD-25).

All three then run the compile test. cargo-semver-checks sees neither
`#[doc(hidden)]` items nor, in 0.51.0, type aliases, so a listing names the ones
consumers import under "Doc-hidden paths consumers import": docs/rename-manifest.md
for `manifest`, and for `release` while the workspace's stage is S2, and
docs/s1-removals.md otherwise. For each crate and set of features named there, a
scratch crate under target/api-removals/ imports every listed path not marked
**Removed**, with only those features, and is checked on its own, so one entry
cannot compile through another's features.

A removal is a lint from the `*_missing` family, `macro_no_longer_exported`, the
`*_now_doc_hidden` family, or a trait's removed associated constant or type.

A member whose package name is not in the baseline's workspace is passed as
`--exclude`, with a warning, since one package missing at the baseline fails the
whole run. cargo-semver-checks skips `publish = false` members of its own accord.

cargo-semver-checks exits 0 when clean, 100 when a lint fails, which this script
reads as data, and 101 on an error of its own, which it never reads as clean. It
names each crate on stderr and reports that crate's lints on stdout, so the two
are read through one pipe, which keeps them in order. Its output is echoed as it
arrives.

Exit codes: 0 when the gate passes; 1 when it fails (an unlisted removal, a listed
path that no longer compiles, in `release` any lint failure, or under `manifest
--strict` any finding); 2 when it cannot decide (cargo-semver-checks or rustdoc
errored, a report or rustdoc JSON of an unread format could not be read, the
baseline is not in the checkout, a listing is malformed, a manifest row has no
reason or new path, or a file or command could not be read), as argparse also does
for bad arguments.

Stdlib only.

Usage:
    python3 scripts/api_removals.py manifest --baseline v0.39.0 [--write] [--strict]
        [--expand FILE] [--no-semver-checks]
    python3 scripts/api_removals.py check --baseline v0.38.1 [--release-type patch]
    python3 scripts/api_removals.py release --baseline v0.38.1
"""
from __future__ import annotations

import argparse
import difflib
import json
import re
import shutil
import subprocess
import sys
import tomllib
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import NamedTuple

sys.path.insert(0, str(Path(__file__).resolve().parent))
import public_api  # noqa: E402
import publish_order  # noqa: E402

REPO = publish_order.REPO
LISTING = "docs/s1-removals.md"
REMOVED_SECTION = "Removed"
HIDDEN_SECTION = "Doc-hidden paths consumers import"
MANIFEST = "docs/rename-manifest.md"
PATHS_SECTION = "Paths"
FEATURES_SECTION = "Features"
ARROW = "→"
# The generated regions of the manifest, in the order the file holds them.
REGIONS = ("unresolved", "stale", "changed-in-place")
SCRATCH = "api-removals"
CRATE_MISSING = "crate_missing"

REMOVAL_LINT = re.compile(
    r"^(?:\w+_missing|macro_no_longer_exported|\w+_now_doc_hidden"
    r"|trait_removed_associated_(?:constant|type))$")

ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
CHECKING = re.compile(r"^\s*Checking (?P<crate>\S+) v\S+ -> v\S+")
LINT_HEADER = re.compile(r"^--- (?P<level>failure|warning) (?P<lint>\w+): .* ---$")
# Where a "Failed in:" item was, as the end of the line in each of the tool's
# templates: ", previously in file F:N", " previously in file F:N", ", previously
# at F:N", " in file F:N" and " in F:N". The path holds no space, so only the last
# word can match, and an item named `at` or `in` is never cut. `feature_missing`
# names no file.
LOCATION = re.compile(
    r"(?:, previously in file| previously in file|, previously at| in file| in) "
    r"(?P<path>\S+):\d+$")
# The tree cargo-semver-checks extracts the baseline into.
EXTRACTED = re.compile(r"^.*/semver-checks/git-[^/]+/[0-9a-f]+/")
CODE_SPAN = re.compile(r"(?<!`)(`+)(?!`)(.+?)(?<!`)\1(?!`)")
RUST_PATH = re.compile(r"^[a-z_][a-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+$")
GROUPED_PATH = re.compile(r"^(?P<prefix>[a-z_][a-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)"
                          r"::\{(?P<names>[^{}]*)\}$")
IDENTIFIER = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
FEATURE = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.-]*$")
# A manifest row's path: a crate's library name, then any segments, a tuple
# field's index among them.
ROW_PATH = re.compile(r"^[a-z_][a-z0-9_]*(?:::(?:[A-Za-z_][A-Za-z0-9_]*|[0-9]+))*$")
# A `## Features` row's key: a package name, or a package name and a feature.
ROW_FEATURE = re.compile(r"^[a-z0-9_][a-z0-9_-]*(?:/[A-Za-z0-9_][A-Za-z0-9_.-]*)?$")

# A runner takes a command and returns the finished process, its output as text.
Runner = Callable[[list], subprocess.CompletedProcess]


class RemovalsError(Exception):
    """The gate cannot decide. The message says why."""

    exit_code = 2


class Lint(NamedTuple):
    crate: str
    lint: str
    # `failure` or `warning`, as the tool printed it.
    level: str
    # The "Failed in:" line, without its location.
    item: str = ""
    # The item's source file, relative to its crate.
    file: str | None = None

    @property
    def key(self) -> str:
        key = f"{self.crate} {self.lint}"
        if self.item:
            key += f": {self.item}"
        if self.file:
            key += f" ({self.file})"
        return key

    @property
    def is_removal(self) -> bool:
        return REMOVAL_LINT.match(self.lint) is not None


class Report(NamedTuple):
    # The crates the tool named in a `Checking` line, in its order.
    checked: list[str]
    lints: list[Lint]


class HiddenPath(NamedTuple):
    path: str
    features: tuple[str, ...] = ()
    removed: bool = False

    @property
    def crate(self) -> str:
        """The library name the path starts with, such as `polyoxide_sports`."""
        return self.path.split("::", 1)[0]


class Listing(NamedTuple):
    baseline: str
    removed: frozenset[str]
    hidden: tuple[HiddenPath, ...]


# --- running commands --------------------------------------------------------


def run_command(command: list) -> subprocess.CompletedProcess:
    """Run a short command in the repository, capturing its output."""
    try:
        return publish_order.run_command(command)
    except publish_order.PublishOrderError as err:
        raise RemovalsError(str(err)) from err


def _echo(line: str) -> str | None:
    """What the log shows for one line of output: a cargo JSON message as rustc
    rendered it, or nothing for the other JSON messages, and any other line as is."""
    if not line.startswith('{"reason":'):
        return line
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        return line
    if message.get("reason") == "compiler-message":
        return (message.get("message") or {}).get("rendered")
    return None


def stream_command(command: list) -> subprocess.CompletedProcess:
    """Run a long command in the repository, echoing its output as it arrives.

    stderr shares stdout's pipe, so the returned `stdout` holds both in the order
    they were written, and `stderr` is empty.
    """
    try:
        process = subprocess.Popen(command, cwd=REPO, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True,
                                   encoding="utf-8", errors="replace")
    except OSError as err:
        raise RemovalsError(f"could not run `{command[0]}`: {err}") from err
    lines = []
    with process:
        for line in process.stdout:
            shown = _echo(line)
            if shown:
                sys.stdout.write(shown if shown.endswith("\n") else shown + "\n")
                sys.stdout.flush()
            lines.append(line)
    return subprocess.CompletedProcess(command, process.returncode, "".join(lines), "")


def _failed(command: list, result: subprocess.CompletedProcess) -> RemovalsError:
    detail = (result.stderr or "").strip() or (result.stdout or "").strip() or "no output"
    return RemovalsError(f"`{' '.join(command)}` exited {result.returncode}: {detail}")


# --- reading the tool's report -----------------------------------------------


@dataclass
class _Block:
    crate: str
    lint: str
    level: str
    items: list[tuple[str, str | None]] = field(default_factory=list)


def split_location(line: str) -> tuple[str, str | None]:
    """A "Failed in:" line's item, and the path it names, if any."""
    match = LOCATION.search(line)
    if match is None:
        return line, None
    return line[:match.start()], match["path"]


def crate_file(path: str, crate_dir: str | None) -> str:
    """`path` relative to its crate's directory.

    The baseline's files sit in the tree the tool extracts, and the current
    crate's in the checkout, so both prefixes go. A crate the baseline kept in
    another directory keeps that directory in its path.
    """
    rest = EXTRACTED.sub("", path, count=1)
    if crate_dir:
        marker = f"{crate_dir}/"
        if rest.startswith(marker):
            return rest[len(marker):]
        index = rest.rfind(f"/{marker}")
        if index != -1:
            return rest[index + 1 + len(marker):]
    return rest


def parse_report(output: str, crate_dirs: dict[str, str] | None = None) -> Report:
    """Every lint the report names, one per item under its "Failed in:".

    `crate_dirs` maps each crate to its directory in the workspace, which is cut
    from the item's path. The tool prints an item once for each path it is
    importable at, and some templates name only the item, so an item can appear
    twice; it is kept once. Refuses a lint reported before any crate is named, and
    a lint with no item it can read: either means the report's shape changed, and
    reading on would let a removal through unnamed.
    """
    crate_dirs = crate_dirs or {}
    checked: list[str] = []
    blocks: list[_Block] = []
    crate = None
    reading = False
    for line in ANSI.sub("", output).splitlines():
        line = line.rstrip()
        if (match := CHECKING.match(line)) is not None:
            crate, reading = match["crate"], False
            checked.append(crate)
        elif (match := LINT_HEADER.match(line)) is not None:
            if crate is None:
                raise RemovalsError(f"cargo-semver-checks reported {match['lint']} before "
                                    f"naming a crate; its output format may have changed")
            blocks.append(_Block(crate, match["lint"], match["level"]))
            reading = False
        elif line == "Failed in:" and blocks:
            reading = True
        elif reading and line.startswith("  ") and line.strip():
            blocks[-1].items.append(split_location(line.strip()))
        else:
            reading = False
    empty = [f"{b.crate} {b.lint}" for b in blocks if not b.items]
    if empty:
        raise RemovalsError(f"could not read the items cargo-semver-checks reported for "
                            f"{', '.join(empty)}; its output format may have changed")
    lints = [Lint(b.crate, b.lint, b.level, item,
                  crate_file(path, crate_dirs.get(b.crate)) if path else None)
             for b in blocks for item, path in b.items]
    return Report(checked, list(dict.fromkeys(lints)))


# --- docs/s1-removals.md -----------------------------------------------------


def code_spans(text: str) -> list[re.Match]:
    """Each Markdown code span in `text`, in order. Group 2 is its raw content."""
    return list(CODE_SPAN.finditer(text))


def _content(span: re.Match) -> str:
    content = span[2]
    if len(content) > 1 and content.startswith(" ") and content.endswith(" "):
        content = content[1:-1]
    return content


def _front_matter(text: str, label: str) -> tuple[dict[str, str], str]:
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        raise RemovalsError(f"{label} must start with front matter naming its baseline")
    try:
        end = next(i for i, line in enumerate(lines[1:], start=1) if line.strip() == "---")
    except StopIteration:
        raise RemovalsError(f"{label}: its front matter never ends") from None
    fields = {}
    for line in lines[1:end]:
        key, colon, value = line.partition(":")
        if colon and key.strip():
            fields[key.strip()] = value.strip()
    return fields, "\n".join(lines[end + 1:])


def _sections(body: str) -> dict[str, list[str]]:
    """Each `## ` section's list entries, without their `- `."""
    sections: dict[str, list[str]] = {}
    current = None
    for line in body.split("\n"):
        if line.startswith("## "):
            current = line[3:].strip()
            sections.setdefault(current, [])
        elif current is not None and line.startswith("- "):
            sections[current].append(line[2:].strip())
    return sections


def _expand(path: str, label: str) -> list[str]:
    """`a::b::{C, D}` as `a::b::C` and `a::b::D`; any other path as itself."""
    match = GROUPED_PATH.match(path)
    if match is None:
        paths = [path]
    else:
        names = [name.strip() for name in match["names"].split(",")]
        if not all(IDENTIFIER.match(name) for name in names):
            raise RemovalsError(f"{label}: {path!r} must group plain names, as in "
                                f"`a::b::{{C, D}}`")
        paths = [f"{match['prefix']}::{name}" for name in names]
    for expanded in paths:
        if not RUST_PATH.match(expanded):
            raise RemovalsError(f"{label}: a `## {HIDDEN_SECTION}` entry must start with a "
                                f"crate path in backticks, not {path!r}")
    return paths


def parse_listing(text: str, label: str = LISTING) -> Listing:
    """The baseline, the listed removal keys, and the paths the compile test imports."""
    fields, body = _front_matter(text, label)
    baseline = fields.get("baseline")
    if not baseline:
        raise RemovalsError(f"{label}: its front matter has no `baseline:`")
    sections = _sections(body)
    for name in (REMOVED_SECTION, HIDDEN_SECTION):
        if name not in sections:
            raise RemovalsError(f"{label} has no `## {name}` section")
    removed = set()
    for entry in sections[REMOVED_SECTION]:
        spans = code_spans(entry)
        if not entry.startswith("`") or not spans:
            raise RemovalsError(f"{label}: a `## {REMOVED_SECTION}` entry must start with "
                                f"its key in backticks, not {entry!r}")
        if not entry[spans[0].end():].strip():
            raise RemovalsError(f"{label}: the `## {REMOVED_SECTION}` entry for "
                                f"{_content(spans[0])!r} must say, after its key, which "
                                f"story removed it and what a consumer uses instead")
        removed.add(_content(spans[0]))
    return Listing(baseline, frozenset(removed), _hidden_paths(sections[HIDDEN_SECTION], label))


def _hidden_paths(entries: list[str], label: str) -> tuple[HiddenPath, ...]:
    """The `## Doc-hidden paths consumers import` entries, groups expanded."""
    hidden = []
    for entry in entries:
        head, marker, _ = entry.partition("**Removed**")
        spans = [_content(span) for span in code_spans(head)]
        if not entry.startswith("`") or not spans:
            raise RemovalsError(f"{label}: a `## {HIDDEN_SECTION}` entry must start with a "
                                f"crate path in backticks, not {entry!r}")
        bad = [f for f in spans[1:] if not FEATURE.match(f)]
        if bad:
            raise RemovalsError(f"{label}: {spans[0]} lists {', '.join(map(repr, bad))}, "
                                f"which are not feature names")
        hidden += [HiddenPath(path, tuple(spans[1:]), bool(marker))
                   for path in _expand(spans[0], label)]
    return tuple(hidden)


def read_listing(root: Path) -> Listing:
    path = root / LISTING
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as err:
        raise RemovalsError(f"could not read {LISTING}: {err}") from err
    return parse_listing(text)


# --- docs/rename-manifest.md -------------------------------------------------


class Row(NamedTuple):
    """One manifest row: an old path, and either the path it moved to or why it went."""

    old: str
    new: str | None
    reason: str | None

    def covers(self, path: str, separator: str = "::") -> bool:
        return path == self.old or path.startswith(self.old + separator)

    def maps(self, path: str) -> str:
        """`path`, which this row covers, under the row's new path."""
        return f"{self.new}{path[len(self.old):]}"


class Manifest(NamedTuple):
    baseline: str
    # The `## Paths` rows.
    rows: tuple[Row, ...]
    # The `## Features` rows, keyed `<crate>/<feature>` or `<crate>`.
    features: tuple[Row, ...] = ()
    hidden: tuple[HiddenPath, ...] = ()
    # Each generated region's body, by id, as the file holds it.
    regions: dict[str, str] = {}


def longest_row(rows: tuple[Row, ...], path: str, separator: str = "::") -> Row | None:
    """The row whose old path is the longest prefix of `path` at a `separator`."""
    covering = [row for row in rows if row.covers(path, separator)]
    return max(covering, key=lambda row: len(row.old), default=None)


def _region_markers(region: str) -> tuple[str, str]:
    return (f"<!-- manifest:begin {region} -->", f"<!-- manifest:end {region} -->")


def split_regions(text: str, label: str = MANIFEST) -> tuple[str, dict[str, str]]:
    """`text` with each generated region's body cut out, and the bodies by id.

    Each region must appear exactly once, its end after its begin, with its markers
    on lines of their own. A body is the text between the two marker lines.
    """
    regions = {}
    rest = text
    for region in REGIONS:
        begin, end = _region_markers(region)
        if text.count(begin) != 1 or text.count(end) != 1:
            raise RemovalsError(f"{label} must hold `{begin}` and `{end}` once each")
        start = rest.find(begin + "\n")
        stop = rest.find("\n" + end, start)
        if start == -1 or stop == -1:
            raise RemovalsError(f"{label}: the {region} region's markers must be on lines of "
                                f"their own, its end after its begin")
        body_start = start + len(begin) + 1
        regions[region] = rest[body_start:stop + 1]
        rest = rest[:body_start] + rest[stop + 1:]
    return rest, regions


def _row(entry: str, section: str, pattern: re.Pattern, label: str) -> Row:
    spans = code_spans(entry)
    if not entry.startswith("`") or not spans:
        raise RemovalsError(f"{label}: a `## {section}` row must start with its old path in "
                            f"backticks, not {entry!r}")
    old = _content(spans[0])
    if not pattern.match(old):
        raise RemovalsError(f"{label}: {old!r} in `## {section}` is not a "
                            f"{'path' if section == PATHS_SECTION else 'crate or feature'}")
    rest = entry[spans[0].end():].strip()
    if not rest.startswith(ARROW):
        raise RemovalsError(f"{label}: the `## {section}` row for {old!r} must follow it "
                            f"with `{ARROW}`, then the new path in backticks or a reason")
    rest = rest[len(ARROW):].strip()
    if rest.startswith("`"):
        span = CODE_SPAN.match(rest)
        new = _content(span) if span else ""
        if not pattern.match(new):
            raise RemovalsError(f"{label}: the `## {section}` row for {old!r} maps it to "
                                f"{new or rest!r}, which is not a "
                                f"{'path' if section == PATHS_SECTION else 'crate or feature'}")
        return Row(old, new, None)
    if not rest:
        raise RemovalsError(f"{label}: the `## {section}` row for {old!r} must say, after "
                            f"`{ARROW}`, the new path in backticks or why it went")
    return Row(old, None, rest)


def _rows(entries: list[str], section: str, pattern: re.Pattern,
          label: str) -> tuple[Row, ...]:
    rows = tuple(_row(entry, section, pattern, label) for entry in entries)
    seen: set[str] = set()
    for row in rows:
        if row.old in seen:
            raise RemovalsError(f"{label}: {row.old!r} has two `## {section}` rows")
        seen.add(row.old)
    return rows


def _refuse_wrapped_rows(body: str, label: str) -> None:
    """A row is one line: `_sections` reads only an entry's first line, so a row
    wrapped onto an indented line would lose its end unseen."""
    section = None
    for line in body.split("\n"):
        if line.startswith("## "):
            section = line[3:].strip()
        elif (section in (PATHS_SECTION, FEATURES_SECTION) and line[:1] in (" ", "\t")
              and line.strip()):
            raise RemovalsError(f"{label}: `## {section}` continues a row onto the indented "
                                f"line {line.strip()!r}; a row is one line")


def parse_manifest(text: str, label: str = MANIFEST) -> Manifest:
    """The baseline, the path and feature rows, the paths the compile test imports,
    and the generated regions as the file holds them."""
    fields, body = _front_matter(text, label)
    baseline = fields.get("baseline")
    if not baseline:
        raise RemovalsError(f"{label}: its front matter has no `baseline:`")
    body, regions = split_regions(body, label)
    _refuse_wrapped_rows(body, label)
    sections = _sections(body)
    for name in (PATHS_SECTION, FEATURES_SECTION, HIDDEN_SECTION):
        if name not in sections:
            raise RemovalsError(f"{label} has no `## {name}` section")
    return Manifest(baseline,
                    _rows(sections[PATHS_SECTION], PATHS_SECTION, ROW_PATH, label),
                    _rows(sections[FEATURES_SECTION], FEATURES_SECTION, ROW_FEATURE, label),
                    _hidden_paths(sections[HIDDEN_SECTION], label),
                    regions)


def read_manifest(root: Path) -> Manifest:
    path = root / MANIFEST
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as err:
        raise RemovalsError(f"could not read {MANIFEST}: {err}") from err
    return parse_manifest(text)


# --- the baseline's workspace ------------------------------------------------


def verify_baseline(baseline: str, run: Runner) -> None:
    command = ["git", "rev-parse", "--verify", "--quiet", f"{baseline}^{{commit}}"]
    if run(command).returncode != 0:
        raise RemovalsError(
            f"the baseline {baseline} is not a commit in this checkout. If it is a tag "
            f"that exists, fetch it: in a workflow, actions/checkout with `fetch-depth: 0` "
            f"fetches every tag.")


def _toml_at(rev: str, path: str, run: Runner) -> dict:
    command = ["git", "show", f"{rev}:{path}"]
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result)
    try:
        return tomllib.loads(result.stdout)
    except tomllib.TOMLDecodeError as err:
        raise RemovalsError(f"{rev}:{path} is not valid TOML: {err}") from err


def baseline_members(rev: str, run: Runner) -> dict[str, bool]:
    """Each package in the workspace at `rev`, by name, and whether it was publishable.

    By name, not by directory, so a crate that moved is still found and a crate
    that was renamed counts as one removed and one added.
    """
    root = _toml_at(rev, "Cargo.toml", run)
    workspace = root.get("workspace", {})
    inherited = workspace.get("package", {}).get("publish", True)
    manifests = [("Cargo.toml", root)] if "package" in root else []
    for member in workspace.get("members", []):
        if any(c in member for c in "*?["):
            raise RemovalsError(f"{rev}'s workspace lists the glob {member!r}, which this "
                                f"script cannot expand; list its members by path")
        path = f"{member.rstrip('/')}/Cargo.toml"
        manifests.append((path, _toml_at(rev, path, run)))
    members = {}
    for path, manifest in manifests:
        package = manifest.get("package", {})
        name = package.get("name")
        if not isinstance(name, str):
            raise RemovalsError(f"{rev}:{path} has no [package] name")
        publish = package.get("publish", True)
        if isinstance(publish, dict) and publish.get("workspace"):
            publish = inherited
        members[name] = publish is not False and publish != []
    return members


# --- running cargo-semver-checks ---------------------------------------------


def semver_command(baseline: str, release_type: str | None, excluded: list[str]) -> list[str]:
    command = ["cargo", "semver-checks", "--workspace", "--baseline-rev", baseline]
    if release_type:
        command += ["--release-type", release_type]
    command += ["--color", "never"]
    for name in excluded:
        command += ["--exclude", name]
    return command


def _has_library(package: dict) -> bool:
    return any(kind in ("lib", "rlib", "dylib", "proc-macro")
               for target in package.get("targets", []) for kind in target.get("kind", []))


def _crate_dirs(packages: list[dict], root: Path) -> dict[str, str]:
    dirs = {}
    for package in packages:
        directory = Path(package["manifest_path"]).resolve().parent
        try:
            dirs[package["name"]] = directory.relative_to(root.resolve()).as_posix()
        except ValueError:
            continue
    return dirs


def semver_checks(baseline: str, release_type: str | None, packages: list[dict], root: Path,
                  run: Runner, stream: Runner) -> Report:
    """cargo-semver-checks' report on every member present at `baseline`, plus a
    `crate_missing` failure for each publishable crate the baseline has and this
    workspace does not.

    Refuses a report that is not the tool's own verdict: an exit other than 0 or
    100, an exit that disagrees with the failures read, or a crate it was given
    and never named.
    """
    verify_baseline(baseline, run)
    before = baseline_members(baseline, run)
    names = {p["name"] for p in packages}
    excluded = sorted(names - set(before))
    for name in excluded:
        print(f"::warning::{name} is not a package in {baseline}'s workspace, so "
              f"cargo-semver-checks does not check it", file=sys.stderr)
    gone = sorted(name for name, publishable in before.items()
                  if publishable and name not in names)
    command = semver_command(baseline, release_type, excluded)
    result = stream(command)
    if result.returncode not in (0, 100):
        raise RemovalsError(f"`{' '.join(command)}` exited {result.returncode}, which is an "
                            f"error of its own, not a verdict; see its output above")
    report = parse_report(result.stdout, _crate_dirs(packages, root))
    failed = any(lint.level == "failure" for lint in report.lints)
    if failed != (result.returncode == 100):
        raise RemovalsError(f"cargo-semver-checks exited {result.returncode}, but "
                            f"{'a' if failed else 'no'} failure could be read from its "
                            f"report; its output format may have changed")
    expected = {p["name"] for p in packages
                if p.get("publish") != [] and _has_library(p) and p["name"] not in excluded}
    missing = sorted(expected - set(report.checked))
    if missing:
        raise RemovalsError(f"cargo-semver-checks never named {', '.join(missing)}, so "
                            f"nothing it reported can be read as covering them")
    return Report(report.checked,
                  report.lints + [Lint(name, CRATE_MISSING, "failure") for name in gone])


# --- the compile test --------------------------------------------------------


def scratch_manifest(package: str, path: Path, features: tuple[str, ...]) -> str:
    entry = f"path = {json.dumps(path.as_posix())}"
    if features:
        entry += f", features = {json.dumps(list(features))}"
    return "\n".join([
        "# Written by scripts/api_removals.py; do not edit.",
        "[package]",
        f'name = "{SCRATCH}-check"',
        'version = "0.0.0"',
        'edition = "2021"',
        "publish = false",
        "",
        "[dependencies]",
        f"{package} = {{ {entry} }}",
        "",
        "# Its own workspace, so cargo does not look for it in the repository's.",
        "[workspace]",
        "",
    ])


SCRATCH_PREAMBLE = [
    "//! Written by scripts/api_removals.py from a listing's doc-hidden paths; do not edit.",
    "//! Each line imports one path cargo-semver-checks cannot see, so a path that",
    "//! no longer resolves fails this build on its own line.",
    "#![allow(unused_imports)]",
]


def scratch_source(paths: list[str]) -> tuple[str, dict[int, str]]:
    """A scratch crate's lib.rs, and which path each line imports."""
    lines = list(SCRATCH_PREAMBLE)
    by_line = {}
    for path in paths:
        lines.append(f"use {path} as _;")
        by_line[len(lines)] = path
    return "\n".join(lines) + "\n", by_line


def error_lines(output: str, manifest: Path) -> set[int]:
    """The lines of `manifest`'s own src/lib.rs that rustc reported an error on.

    Read from cargo's JSON messages, so an error or warning in a dependency, which
    may well be at its own `src/lib.rs`, is never taken for one of these lines.
    """
    lines = set()
    for text in output.splitlines():
        if not text.startswith("{"):
            continue
        try:
            message = json.loads(text)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-message" or not message.get("manifest_path"):
            continue
        if Path(message["manifest_path"]).resolve() != manifest.resolve():
            continue
        diagnostic = message.get("message") or {}
        if diagnostic.get("level") != "error":
            continue
        for span in diagnostic.get("spans") or []:
            if span.get("is_primary") and span.get("file_name") == "src/lib.rs":
                lines.add(span["line_start"])
    return lines


def _features_label(features: tuple[str, ...]) -> str:
    return " and ".join(f"`{f}`" for f in features) if features else "default features"


def compile_hidden(paths: tuple[HiddenPath, ...], metadata: dict, root: Path,
                   stream: Runner, listing: str = LISTING) -> int:
    """Check the listed paths not marked removed. 0 if every group builds, else 1.

    One scratch crate per (crate, features) pair, each built on its own, so a
    path builds with exactly the features its entry names. They share one target
    directory.
    """
    live = [p for p in paths if not p.removed]
    if not live:
        return 0
    members = {p["name"].replace("-", "_"): p for p in publish_order.members(metadata)}
    unknown = sorted({p.path for p in live if p.crate not in members})
    if unknown:
        print(f"::error::{listing} lists paths in crates the workspace no longer has: "
              f"{', '.join(unknown)}. If they were removed on purpose, mark them **Removed**.",
              file=sys.stderr)
        return 1
    groups: dict[tuple[str, tuple[str, ...]], list[str]] = {}
    for hidden in live:
        name = members[hidden.crate]["name"]
        groups.setdefault((name, tuple(sorted(set(hidden.features)))), []).append(hidden.path)
    scratch = Path(metadata["target_directory"]) / SCRATCH
    status = 0
    for (name, features), group in groups.items():
        package = next(p for p in members.values() if p["name"] == name)
        directory = scratch / "+".join([name, *features])
        (directory / "src").mkdir(parents=True, exist_ok=True)
        manifest = directory / "Cargo.toml"
        crate_dir = Path(package["manifest_path"]).resolve().parent
        manifest.write_text(scratch_manifest(name, crate_dir, features), encoding="utf-8")
        source, by_line = scratch_source(group)
        (directory / "src" / "lib.rs").write_text(source, encoding="utf-8")
        # The workspace's lockfile, so the build uses the versions the workspace pins.
        lockfile = root / "Cargo.lock"
        if lockfile.is_file():
            shutil.copyfile(lockfile, directory / "Cargo.lock")
        label = f"{name} with {_features_label(features)}"
        print(f"Compiling {len(group)} listed paths from {label}", file=sys.stderr)
        command = ["cargo", "check", "--manifest-path", str(manifest),
                   "--target-dir", str(scratch / "target"),
                   "--message-format", "json", "--color", "never"]
        result = stream(command)
        if result.returncode == 0:
            continue
        status = 1
        failing = [by_line[n] for n in sorted(error_lines(result.stdout, manifest))
                   if n in by_line]
        if failing:
            print(f"::error::Paths listed in {listing} that no longer compile from {label}: "
                  f"{', '.join(failing)}. A consumer imports each of them. If one was "
                  f"removed on purpose, mark it **Removed** there.", file=sys.stderr)
        else:
            print(f"::error::The compile test of {label} failed, but not on a listed path; "
                  f"see cargo's output above.", file=sys.stderr)
    return status


# --- the subcommands ---------------------------------------------------------


def raises_minor(baseline: str, current: str) -> bool:
    """Whether `current` may break `baseline`'s API: a new 0.x minor or a new major.

    A baseline that is not a version tag allows nothing.
    """
    try:
        old = publish_order._semver_key(baseline.removeprefix("v"))[0]
        new = publish_order._semver_key(current)[0]
    except publish_order.PublishOrderError:
        return False
    if len(old) < 2 or len(new) < 2:
        return False
    if old[0] == new[0] == 0:
        return new[1] > old[1]
    return new[0] > old[0]


def check(baseline: str, release_type: str, root: Path = REPO, run: Runner = run_command,
          stream: Runner = stream_command) -> int:
    listing = read_listing(root)
    if listing.baseline != baseline:
        raise RemovalsError(f"{LISTING} lists removals against {listing.baseline}, not "
                            f"{baseline}; the two must name the same baseline")
    metadata = publish_order.cargo_metadata(root / "Cargo.toml", run)
    report = semver_checks(baseline, release_type, publish_order.members(metadata), root,
                           run, stream)
    removals = {lint.key for lint in report.lints if lint.is_removal}
    others = {lint.key for lint in report.lints if not lint.is_removal}
    status = 0
    unlisted = sorted(removals - listing.removed)
    if unlisted:
        print(f"::error::Public items removed since {baseline} that {LISTING} does not list. "
              f"Restore each one, or add its key to the list's `## {REMOVED_SECTION}` "
              f"section:", file=sys.stderr)
        for key in unlisted:
            print(f"  {key}", file=sys.stderr)
        status = 1
    stale = sorted(listing.removed - removals)
    if stale:
        print(f"::warning::Listed in {LISTING} but not reported by cargo-semver-checks, "
              f"so either restored or keyed differently now:", file=sys.stderr)
        for key in stale:
            print(f"  {key}", file=sys.stderr)
    print(f"{len(removals)} removals ({len(removals) - len(unlisted)} listed), and "
          f"{len(others)} other changes, which S1 allows.", file=sys.stderr)
    return max(status, compile_hidden(listing.hidden, metadata, root, stream))


def release(baseline: str, root: Path = REPO, run: Runner = run_command,
            stream: Runner = stream_command) -> int:
    # Through S2 the paths S1 listed move, and the manifest lists them where they go.
    if workspace_stage(root) == "S2":
        hidden, listing = read_manifest(root).hidden, MANIFEST
    else:
        hidden, listing = read_listing(root).hidden, LISTING
    metadata = publish_order.cargo_metadata(root / "Cargo.toml", run)
    report = semver_checks(baseline, None, publish_order.members(metadata), root, run, stream)
    current = publish_order.workspace_version(metadata)
    failures = {lint.key for lint in report.lints
                if lint.level == "failure" and lint.lint != CRATE_MISSING}
    if not raises_minor(baseline, current):
        failures |= {lint.key for lint in report.lints if lint.lint == CRATE_MISSING}
    status = 0
    if failures:
        print(f"::error::Changes since {baseline} that a bump to {current} does not allow. "
              f"A release that removes or breaks a public item must raise the 0.x minor "
              f"(AD-25):", file=sys.stderr)
        for key in sorted(failures):
            print(f"  {key}", file=sys.stderr)
        status = 1
    return max(status, compile_hidden(hidden, metadata, root, stream, listing))


# --- the S2 gate -------------------------------------------------------------

# cargo-semver-checks' removal lints the path diff cannot see: the path stays,
# and only its visibility in the docs changes.
NOW_HIDDEN = re.compile(r"^\w+_now_doc_hidden$")
# The regenerated manifest, under the target directory, which CI uploads.
REGENERATED = "rename-manifest.md"


def is_changed_in_place(lint: Lint) -> bool:
    return not lint.is_removal or NOW_HIDDEN.match(lint.lint) is not None


RESOLVED = "resolved"
NO_ROW = "no row"
POINTS_NOWHERE = "points nowhere"
MAPS_TO_HIDDEN = "maps to a doc-hidden path"


class Entry(NamedTuple):
    """A baseline path, or a baseline `<crate>/<feature>`, the commit does not have."""

    kind: str
    path: str
    hidden: bool
    # The row with the longest old path covering it, if any.
    row: Row | None
    # Where that row maps it, if it is a path row.
    mapped: str | None
    # `resolved`, `no row`, `points nowhere`, `exists as <kind> at HEAD` or
    # `maps to a doc-hidden path`.
    status: str

    @property
    def resolved(self) -> bool:
        return self.status == RESOLVED

    @property
    def why(self) -> str:
        """Why it is unresolved, naming where its row maps it."""
        return f"{self.status}, {_span(self.mapped)}" if self.mapped else self.status


def _span(text: str) -> str:
    """`text` as one Markdown code span, whatever backticks it holds."""
    ticks = max((len(run) for run in re.findall(r"`+", text)), default=0)
    fence, pad = "`" * (ticks + 1), " " if ticks else ""
    return f"{fence}{pad}{text}{pad}{fence}"


def _prefixes(keys, separator: str) -> set[str]:
    """Every key and each of its prefixes at `separator`."""
    found = set()
    for key in keys:
        parts = key.split(separator)
        found.update(separator.join(parts[:n]) for n in range(1, len(parts) + 1))
    return found


def resolve_entries(base: public_api.Snapshot, head: public_api.Snapshot,
                    manifest: Manifest) -> list[Entry]:
    """Every entry, paths by path then kind and then features, each resolved by the
    row with the longest old path that covers it. A reason row resolves what it
    covers. A path row resolves an entry it maps to a path the commit has under the
    same kind, unless it maps a public path onto a doc-hidden one, which only a
    reason row of its own resolves."""
    head_kinds: dict[str, list[str]] = {}
    for kind, path in head.paths:
        head_kinds.setdefault(path, []).append(kind)
    entries = []
    gone = sorted((key for key in base.paths if key not in head.paths),
                  key=lambda key: (key[1], key[0]))
    for kind, path in gone:
        hidden = base.paths[(kind, path)]
        row = longest_row(manifest.rows, path)
        mapped = row.maps(path) if row is not None and row.new is not None else None
        if row is None:
            status = NO_ROW
        elif mapped is None:
            status = RESOLVED
        elif (kind, mapped) in head.paths:
            status = MAPS_TO_HIDDEN if head.paths[(kind, mapped)] and not hidden else RESOLVED
        elif mapped in head_kinds:
            status = f"exists as {' and '.join(sorted(head_kinds[mapped]))} at HEAD"
        else:
            status = POINTS_NOWHERE
        entries.append(Entry(kind, path, hidden, row, mapped, status))
    for feature in sorted(base.features - head.features):
        row = longest_row(manifest.features, feature, "/")
        mapped = row.maps(feature) if row is not None and row.new is not None else None
        if row is None:
            status = NO_ROW
        else:
            status = RESOLVED if mapped is None or mapped in head.features else POINTS_NOWHERE
        entries.append(Entry("feature", feature, False, row, mapped, status))
    return entries


NO_BASELINE_PATH = "covers no baseline path"
NO_ENTRY = "covers no entry: every path it covers is still at HEAD"


def stale_rows(base: public_api.Snapshot, entries: list[Entry],
               manifest: Manifest) -> list[tuple[str, Row, str]]:
    """(section, row, why) for each row that covers no entry, saying whether it
    covers no baseline path at all or only paths the commit still has."""
    sides = [(PATHS_SECTION, manifest.rows, "::", (path for _, path in base.paths),
              (e.path for e in entries if e.kind != "feature")),
             (FEATURES_SECTION, manifest.features, "/", base.features,
              (e.path for e in entries if e.kind == "feature"))]
    stale = []
    for section, rows, separator, baseline_keys, entry_keys in sides:
        baseline, gone = _prefixes(baseline_keys, separator), _prefixes(entry_keys, separator)
        stale += [(section, row, NO_BASELINE_PATH if row.old not in baseline else NO_ENTRY)
                  for row in rows if row.old not in gone]
    return stale


def render_regions(entries: list[Entry], stale: list[tuple[str, Row, str]],
                   changed: list[str] | None) -> dict[str, str]:
    """Each generated region's body. `changed` is None when cargo-semver-checks did
    not run, and then that region is not rendered."""
    def body(lines: list[str]) -> str:
        return "".join(f"{line}\n" for line in lines) if lines else "None.\n"

    regions = {
        "unresolved": body([f"- {_span(e.path)} {e.kind}{', doc-hidden' if e.hidden else ''}: "
                            f"{e.why}" for e in entries if not e.resolved]),
        "stale": body([f"- {_span(row.old)} in `## {section}`: {why}"
                       for section, row, why in stale]),
    }
    if changed is not None:
        regions["changed-in-place"] = body([f"- {_span(key)}" for key in changed])
    return regions


def replace_regions(text: str, regions: dict[str, str]) -> str:
    """`text` with each region named in `regions` holding its new body."""
    for region, body in regions.items():
        begin, end = _region_markers(region)
        start = text.index(begin + "\n") + len(begin) + 1
        stop = text.index("\n" + end, start - 1) + 1
        text = text[:start] + body + text[stop:]
    return text


def _tsv_field(text: str) -> str:
    return text.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")


def expand_line(entry: Entry) -> str:
    """`old<TAB>new or reason<TAB>kind<TAB>status`, the new path only when it
    resolved, the status with `, doc-hidden` for a hidden path."""
    if entry.row is not None and entry.row.reason is not None:
        target = entry.row.reason
    else:
        target = entry.mapped if entry.resolved and entry.mapped else ""
    status = entry.status + (", doc-hidden" if entry.hidden else "")
    return "\t".join(map(_tsv_field, (entry.path, target, entry.kind, status))) + "\n"


def check_hidden_flags(hidden: tuple[HiddenPath, ...], head: public_api.Snapshot,
                       base: public_api.Snapshot) -> None:
    """Refuse a listing whose doc-hidden paths the walker does not flag hidden.

    Each live path the manifest imports is looked up in the working tree's listing,
    or the baseline's if the tree no longer has it, and must be flagged there; a
    type alias is listed for cargo-semver-checks' blind spot, not for being hidden,
    so an unhidden one is skipped. A rustdoc format that spells the attribute
    another way would otherwise drop every hidden path from the diff unseen.
    """
    def by_path(paths: public_api.Paths) -> dict[str, dict[str, bool]]:
        found: dict[str, dict[str, bool]] = {}
        for (kind, path), flag in paths.items():
            found.setdefault(path, {})[kind] = flag
        return found

    heads, bases = by_path(head.paths), by_path(base.paths)
    missing, unflagged, flagged = [], [], []
    for entry in hidden:
        if entry.removed:
            continue
        kinds = heads.get(entry.path) or bases.get(entry.path)
        if not kinds:
            missing.append(entry.path)
        elif any(kinds.values()):
            flagged.append(entry.path)
        elif "type_alias" not in kinds:
            unflagged.append(entry.path)
    if unflagged and not flagged:
        raise RemovalsError(
            f"{MANIFEST}'s `## {HIDDEN_SECTION}` lists {', '.join(unflagged)}, and the rustdoc "
            f"JSON flags none of them hidden: the walker no longer recognises "
            f"`#[doc(hidden)]` in this rustdoc format, so it cannot tell which entries are "
            f"hidden")
    if missing or unflagged:
        problems = []
        if unflagged:
            problems.append(f"lists {', '.join(unflagged)}, which are no longer doc-hidden")
        if missing:
            problems.append(f"lists {', '.join(missing)}, which neither listing has")
        raise RemovalsError(
            f"{MANIFEST}'s `## {HIDDEN_SECTION}` {' and '.join(problems)}; update that "
            f"section to the paths consumers import now")


def workspace_stage(root: Path) -> str | None:
    """`[workspace.metadata.polyoxide] stage` in the root manifest, if it is set."""
    try:
        manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    except OSError as err:
        raise RemovalsError(f"could not read Cargo.toml: {err}") from err
    return manifest.get("workspace", {}).get("metadata", {}).get("polyoxide", {}).get("stage")


def manifest_gate(baseline: str, *, write: bool = False, strict: bool = False,
                  expand: Path | None = None, semver: bool = True, root: Path = REPO,
                  run: Runner = run_command, stream: Runner = stream_command,
                  snapshot=None) -> int:
    snapshot = snapshot or public_api.snapshot
    manifest = read_manifest(root)
    if manifest.baseline != baseline:
        raise RemovalsError(f"{MANIFEST} maps paths from {manifest.baseline}, not "
                            f"{baseline}; the two must name the same baseline")
    verify_baseline(baseline, run)
    metadata = publish_order.cargo_metadata(root / "Cargo.toml", run)
    scratch = Path(metadata["target_directory"]) / SCRATCH
    base = snapshot(baseline, root, scratch, run, stream)
    head = snapshot(None, root, scratch, run, stream)
    check_hidden_flags(manifest.hidden, head, base)
    changed = None
    if semver:
        report = semver_checks(baseline, "patch", publish_order.members(metadata), root, run,
                               stream)
        changed = sorted({lint.key for lint in report.lints if is_changed_in_place(lint)})
    entries = resolve_entries(base, head, manifest)
    stale = stale_rows(base, entries, manifest)
    regions = render_regions(entries, stale, changed)

    unresolved = [e for e in entries if not e.resolved]
    for entry in unresolved:
        print(f"::warning::{entry.path} ({entry.kind}{', doc-hidden' if entry.hidden else ''}) "
              f"is gone since {baseline}, and {MANIFEST} has {entry.why}", file=sys.stderr)
    for section, row, why in stale:
        print(f"::warning::{MANIFEST}'s `## {section}` row for {row.old} {why}",
              file=sys.stderr)
    outdated = [region for region, body in regions.items() if manifest.regions[region] != body]
    for region in outdated:
        sys.stdout.writelines(difflib.unified_diff(
            manifest.regions[region].splitlines(keepends=True),
            regions[region].splitlines(keepends=True),
            f"{MANIFEST} ({region}, checked in)", f"{MANIFEST} ({region}, regenerated)"))
        if not write:
            print(f"::warning::{MANIFEST}'s `{region}` region is stale; the hub regenerates "
                  f"it with `--write`", file=sys.stderr)

    text = replace_regions((root / MANIFEST).read_text(encoding="utf-8"), regions)
    scratch.mkdir(parents=True, exist_ok=True)
    (scratch / REGENERATED).write_text(text, encoding="utf-8")
    if write and outdated:
        (root / MANIFEST).write_text(text, encoding="utf-8")
    if expand is not None:
        expand.parent.mkdir(parents=True, exist_ok=True)
        expand.write_text("".join(expand_line(e) for e in entries), encoding="utf-8")

    pointing_nowhere = sum(1 for e in unresolved if e.status == POINTS_NOWHERE)
    print(f"{len(entries)} entr{'y' if len(entries) == 1 else 'ies'} since {baseline}: "
          f"{len(entries) - len(unresolved)} "
          f"resolved, {len(unresolved)} unresolved ({pointing_nowhere} pointing nowhere); "
          f"{len(stale)} stale rows; {len(outdated)} stale regions"
          f"{' (rewritten)' if write and outdated else ''}"
          f"{'' if semver else '; cargo-semver-checks not run, so changed-in-place is kept'}.",
          file=sys.stderr)
    status = 0
    if strict and (unresolved or stale or (outdated and not write)):
        print(f"::error::--strict: {MANIFEST} must resolve every entry, hold no stale row and "
              f"have its regions regenerated", file=sys.stderr)
        status = 1
    return max(status, compile_hidden(manifest.hidden, metadata, root, stream, MANIFEST))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Check the public paths and items removed since a baseline against "
                    "docs/rename-manifest.md or docs/s1-removals.md.")
    commands = parser.add_subparsers(dest="command", required=True)
    checking = commands.add_parser(
        "check", help="fail on a removal docs/s1-removals.md does not list")
    checking.add_argument("--baseline", required=True, metavar="REV",
                          help="the git revision to compare with, such as v0.38.1")
    checking.add_argument("--release-type", default="patch",
                          choices=("patch", "minor", "major"),
                          help="passed to cargo-semver-checks; patch, the default, makes "
                               "every lint required")
    releasing = commands.add_parser(
        "release", help="fail on any lint failure against the previous release")
    releasing.add_argument("--baseline", required=True, metavar="REV",
                           help="the previous release's tag, such as v0.38.1")
    mapping = commands.add_parser(
        "manifest", help="report the public paths gone since the S2 baseline, against "
                         "docs/rename-manifest.md")
    mapping.add_argument("--baseline", required=True, metavar="REV",
                         help="the git revision to compare with, such as v0.39.0")
    mapping.add_argument("--write", action="store_true",
                         help="rewrite the manifest's generated regions")
    mapping.add_argument("--strict", action="store_true",
                         help="fail on an unresolved entry, a stale row or a stale region")
    mapping.add_argument("--expand", type=Path, metavar="FILE",
                         help="write each entry as `old<TAB>new or reason<TAB>kind<TAB>status`")
    mapping.add_argument("--no-semver-checks", dest="semver", action="store_false",
                         help="skip cargo-semver-checks, keeping the changed-in-place region")
    return parser


def main(argv: list[str] | None = None, root: Path = REPO, run: Runner = run_command,
         stream: Runner = stream_command, snapshot=None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "check":
            return check(args.baseline, args.release_type, root, run, stream)
        if args.command == "manifest":
            return manifest_gate(args.baseline, write=args.write, strict=args.strict,
                                 expand=args.expand, semver=args.semver, root=root, run=run,
                                 stream=stream, snapshot=snapshot)
        return release(args.baseline, root, run, stream)
    except (RemovalsError, publish_order.PublishOrderError, public_api.PublicApiError) as err:
        print(f"::error::{err}", file=sys.stderr)
        return 2
    # A full disk, an unreadable file or malformed JSON or TOML says nothing about
    # the API, so it must not read as the gate failing.
    except (OSError, ValueError, KeyError) as err:
        print(f"::error::cannot decide: {type(err).__name__}: {err}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
