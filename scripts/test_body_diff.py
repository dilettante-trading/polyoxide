#!/usr/bin/env python3
"""The body diff: every test-context item at a baseline, against its counterpart at HEAD.

Stage S2 moves crates, modules and test files, and a move may change only file
locations and `use` paths (AD-12): test names and assertions stay. This script holds
that. It reads two trees, the baseline revision and the checkout (HEAD, with any edits
not yet committed), and in each finds every test-context item. An item is a fn, const,
static, struct, enum, union, type alias, `extern` item, `macro_rules!` macro or macro
invocation at module level, or a fn, const or type in an `impl` or `trait` block
there (a `method`, `assoc_const` or `assoc_type`), and it is in a test context when
it is

  - in a `.rs` file under a package's `tests/`, in a `[[test]]` or `[[bench]]`
    target, in an `[[example]]` that sets `test = true`, or in a file that a
    test-context `mod x;` declares, by its name or its `#[path]`;
  - in a module whose `cfg` names `test` outside a `not(...)`, so `#[cfg(test)]`,
    `#[cfg(any(test, feature = "test-server"))]` and a file's `#![cfg(test)]` count;
  - or anywhere, when it carries `#[test]`, a `#[<path>::test]` such as
    `#[tokio::test]`, or a `cfg` naming `test`.

Two more kinds are compared besides: each `[[test]]` and `[[example]]` stanza of a
manifest (`target`), keyed by its target's file, by every field but `name` and `path`,
its `required-features` mapped on the baseline side through the manifest's
`## Features` rows and at HEAD without the features `[gates]` drops; and each file
under a `tests/fixtures/` but its Markdown (`fixture`), by its blob id.

The invocations are there because a `macro_rules!` can define a test whose data its
invocation holds (the spec-agreement suites' `agreement! { ... }`). A test is a fn
with a test attribute, or a test attribute inside a macro or an invocation. An item
inside a fn is part of its body, and README and doc-comment doctests are not read.
Each item is keyed by `(file, module path, kind, name)`, its module path being the
inline modules around it within its file, and is compared by its attributes other
than doc comments, its signature and its body, after what gates it: the attributes
of each `mod x;` that declares its file but `#[path]`, its file's inner `cfg`s, and
each enclosing inline module's attributes and inner `cfg`s, without the module
feature gates and cfg predicates `[gates]` drops at HEAD. A module's `const _`
items are named `_#1`, `_#2` and on, in order, its invocations of `m!` are `m!#1` and
on, and its `extern` blocks `block#1` and on.

A method or an associated item is named `<Type>::<name>`: its impl's self type,
`<impl Trait for Type>` for a trait impl, or a trait's own name, the first two
normalised as a body is, so on the baseline side through the manifest's rows. Its
block's attributes and header are compared with it. Two of one name in one module are
told apart by their blocks' attributes, then by `#n` in source order.

Pairing. Files at the two revisions are paired by `git diff -M30% --name-status`,
with `[files]` in ci/test-body-diff.toml overriding it, and an item is paired with
the item of its key in its file's pair. One left over is paired by its
`(module path, kind, name)` when exactly one baseline item and exactly one unpaired
HEAD item have it. Two items of one key in one tree are refused. git pairs only the
files it tracks, so stage a moved file before running this locally.

Normalising, in order, before the two texts of a pair are compared:

  1. Comments, `#[doc = ...]` attributes and every whole `use ...;` statement go, a
     `use` counting as one only where a statement can start.
  2. `crate::` in a library's `src/` becomes the library's name, so a file that moves
     between crates compares by the paths it names. A test target's `crate::` stays.
  3. On the baseline side only, the rename manifest's path rows apply, then `[paths]`,
     which wins on an equal old path: the longest old path at a `::` boundary first.
     Where a row's last segment changes, a path no row covers is renamed at its head,
     in a baseline file that is in the row's crate or names it.
  4. An `include_str!` or `include_bytes!` target becomes its blob's id, so a fixture
     that moved compares by its content.
  5. On the baseline side only, `[literals]` replaces string literals whose content it
     names exactly, a scope matching its own path and those under it at a `/`.
  6. The layout goes: the texts compare token by token, so whitespace is collapsed,
     and so is what rustfmt adds when a line re-wraps under a longer path: a trailing
     comma before a closing bracket, but in a one-element tuple, and the braces
     around a lone expression after `=>` or a closure's `|...|`.

A diff is printed, and fails, unless a `[[drift]]` entry names the item, a DRIFT row of
divergences.md and the diff's sha256. A lost item, one with no counterpart, fails
unless a `[[retired]]` entry names it and a manifest row giving a reason, not a new
path, covers the entry's `row`. Every entry that matches nothing fails: a `[[drift]]`
or `[[retired]]` naming no such diff or lost item, a `[files]` pair naming a file
either tree lacks or a baseline file holding no item, and a `[literals]`, `[paths]` or
`[gates]` entry that replaces or drops nothing. Items new at HEAD are listed.

`--moves` prints, for every paired item whose place changed, `old file:line name ->
new file:line`, the form docs/MUTANTS.md cites tests in.

Exit codes: 0 when every item pairs with an equal body or an acknowledged one; 1 when
one does not, or an entry is stale or invalid; 2 when the diff cannot decide (git
failed, the baseline is not in the checkout, a file does not balance or holds an item
it cannot read, two items share a key, two rows rename one identifier differently,
ci/test-body-diff.toml is malformed or names an item twice, or a file, the config or
the manifest cannot be read).

Stdlib only.

Usage:
    python3 scripts/test_body_diff.py [--baseline REV] [--moves]
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import posixpath
import re
import subprocess
import sys
import tomllib
from collections import Counter, defaultdict
from collections.abc import Callable, Iterable
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import NamedTuple, Protocol

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_registry import CODE, STRING, lex, view  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
CONFIG = "ci/test-body-diff.toml"
DIVERGENCES = "_bmad-output/specs/spec-venue-extensibility/divergences.md"
SIMILARITY = "-M30%"
# What an item can be: Rust items, then a target's manifest stanza and a fixture.
RUST_KINDS = ("fn", "const", "static", "macro", "invocation", "struct", "enum", "union",
              "type", "extern", "method", "assoc_const", "assoc_type")
KINDS = RUST_KINDS + ("target", "fixture")

WORD = re.compile(r"(?:r#)?[^\W\d]\w*")
IDENTIFIER = re.compile(r"\b[^\W\d]\w*")
TEST_ATTRIBUTE = re.compile(r"\s*(?:::\s*)?(?:\w+\s*::\s*)*test\s*(?:\(|$)")
NESTED_TEST = re.compile(r"#\s*\[\s*(?:::\s*)?(?:\w+\s*::\s*)*test\s*[(\]]")
CFG_ATTRIBUTE = re.compile(r"\s*cfg\s*\(")
CFG_GATE = re.compile(r"\s*cfg(?:_attr)?\s*\(")
PATH_ATTRIBUTE = re.compile(r'\s*path\s*=\s*"(?P<path>[^"]*)"\s*$')
DRIFT_ROW = re.compile(r"^\|\s*(R\d+)\s*\|", re.M)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
# Files Cargo builds as a crate root, whose `mod x;` is `x.rs` beside them.
CRATE_ROOT = re.compile(r"^(?:(?:tests|examples|benches|src/bin)/[^/]+\.rs|build\.rs)$")
MOD_RS = ("mod.rs", "lib.rs", "main.rs")


class BodyDiffError(Exception):
    """The diff cannot decide. The message says why."""

    exit_code = 2


class RowLike(Protocol):
    """A rename manifest row, as `api_removals.read_manifest` returns them."""

    old: str
    new: str | None
    reason: str | None


# --- the two trees -----------------------------------------------------------


def git(root: Path, *args: str, stdin: bytes | None = None) -> bytes:
    command = ["git", "-C", str(root), *args]
    try:
        result = subprocess.run(command, input=stdin, capture_output=True)
    except OSError as err:
        raise BodyDiffError(f"could not run git: {err}") from err
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip() or "no output"
        raise BodyDiffError(f"`{' '.join(command[3:])}` exited {result.returncode}: {detail}")
    return result.stdout


def blob_id(data: bytes) -> str:
    """The id git gives a blob of `data`."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


class Tree:
    """One side of the diff: its files, by path from the repository root."""

    label: str

    def paths(self) -> list[str]:
        raise NotImplementedError

    def read(self, path: str) -> str:
        raise NotImplementedError

    def blob(self, path: str) -> str | None:
        raise NotImplementedError


class Baseline(Tree):
    """The tree of a git revision."""

    def __init__(self, root: Path, rev: str):
        self.root, self.label = root, rev
        try:
            git(root, "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}")
        except BodyDiffError:
            raise BodyDiffError(
                f"the baseline {rev} is not a commit in this checkout. If it is a tag that "
                f"exists, fetch it: in a workflow, actions/checkout with `fetch-depth: 0` "
                f"fetches every tag.") from None
        self._blobs: dict[str, str] = {}
        for entry in git(root, "ls-tree", "-r", "-z", "--full-tree", rev).split(b"\0"):
            if not entry:
                continue
            meta, _, path = entry.partition(b"\t")
            _, kind, sha = meta.split(b" ")
            if kind == b"blob":
                self._blobs[path.decode("utf-8")] = sha.decode()
        self._text: dict[str, str] = {}

    def paths(self) -> list[str]:
        return sorted(self._blobs)

    def preload(self, paths: Iterable[str]) -> None:
        """Read `paths` with one `git cat-file --batch`."""
        wanted = [p for p in paths if p in self._blobs and p not in self._text]
        if not wanted:
            return
        out = git(self.root, "cat-file", "--batch",
                  stdin="".join(f"{self._blobs[p]}\n" for p in wanted).encode())
        offset = 0
        for path in wanted:
            header_end = out.index(b"\n", offset)
            size = int(out[offset:header_end].split(b" ")[2])
            start = header_end + 1
            self._text[path] = out[start:start + size].decode("utf-8")
            offset = start + size + 1

    def read(self, path: str) -> str:
        if path not in self._text:
            self.preload([path])
        if path not in self._text:
            raise BodyDiffError(f"{self.label} has no file {path}")
        return self._text[path]

    def blob(self, path: str) -> str | None:
        return self._blobs.get(path)


class Checkout(Tree):
    """The files in the working tree: tracked, and untracked but not ignored."""

    label = "HEAD"

    def __init__(self, root: Path):
        self.root = root
        listed = git(root, "ls-files", "-z", "--cached", "--others", "--exclude-standard")
        self._paths = sorted({p for p in listed.decode("utf-8").split("\0")
                              if p and (root / p).is_file()})

    def paths(self) -> list[str]:
        return self._paths

    def read(self, path: str) -> str:
        return (self.root / path).read_text(encoding="utf-8")

    def blob(self, path: str) -> str | None:
        file = self.root / path
        return blob_id(file.read_bytes()) if file.is_file() else None


def is_fixture(path: str) -> bool:
    """Whether `path` is a fixture: a file under a `tests/fixtures/`, other than the
    Markdown that documents fixtures, such as a PROVENANCE.md."""
    return "/tests/fixtures/" in f"/{path}" and not path.endswith(".md")


def file_pairs(root: Path, baseline: str) -> dict[str, str | None]:
    """Each `.rs` file and fixture git says was renamed since `baseline`, to its new
    path, and each it says was deleted, to `None`. A file absent here kept its path."""
    out = git(root, "diff", SIMILARITY, "-l0", "--name-status", "-z", "--no-ext-diff",
              baseline, "--", "*.rs", ":(glob)**/tests/fixtures/**")
    fields = out.decode("utf-8").split("\0")
    pairs: dict[str, str | None] = {}
    i = 0
    while i < len(fields) and fields[i]:
        status = fields[i]
        if status[0] in "RC":
            if status[0] == "R":
                pairs[fields[i + 1]] = fields[i + 2]
            i += 3
        else:
            if status[0] == "D":
                pairs[fields[i + 1]] = None
            i += 2
    return pairs


# --- packages ----------------------------------------------------------------


@dataclass(frozen=True)
class Stanza:
    """A `[[test]]` or `[[example]]` table of a package's manifest."""

    key: str  # "test" or "example"
    path: str  # its target's file, from the repository root
    fields: tuple[tuple[str, object], ...]  # every field but `name` and `path`


@dataclass(frozen=True)
class Package:
    directory: str  # "" for the repository root
    name: str
    # What `crate::` names in `src/`, or None when the package has no library.
    library: str | None
    # The crate roots Cargo builds, such as `src/lib.rs` and `examples/x.rs`, from
    # the package's directory.
    roots: frozenset[str]
    # The `[[test]]` and `[[bench]]` targets and the examples that set `test = true`,
    # from the package's directory: test contexts, whatever their directory.
    test_targets: frozenset[str]
    stanzas: tuple[Stanza, ...] = ()

    def relative(self, path: str) -> str:
        return path[len(self.directory) + 1:] if self.directory else path


def _target_path(directory: str, kind: str, target: dict, paths: set[str]) -> str | None:
    if isinstance(target.get("path"), str):
        return posixpath.normpath(target["path"])
    name = target.get("name")
    if not isinstance(name, str):
        return None
    for candidate in (f"{kind}/{name}.rs", f"{kind}/{name}/main.rs"):
        if posixpath.join(directory, candidate) in paths:
            return candidate
    return None


def packages(tree: Tree) -> list[Package]:
    """Every package in `tree`, read from each `Cargo.toml` with a `[package]`.

    A manifest that does not parse is skipped with a warning: it may be a test's
    fixture, and a real crate skipped shows up as its items lost.
    """
    paths = set(tree.paths())
    found = []
    for path in sorted(paths):
        if posixpath.basename(path) != "Cargo.toml":
            continue
        try:
            manifest = tomllib.loads(tree.read(path))
        except tomllib.TOMLDecodeError as err:
            print(f"::warning::{tree.label}:{path} is not valid TOML, so its package is "
                  f"skipped: {err}", file=sys.stderr)
            continue
        package = manifest.get("package")
        if not isinstance(package, dict) or not isinstance(package.get("name"), str):
            continue
        directory = posixpath.dirname(path)
        lib = manifest.get("lib") if isinstance(manifest.get("lib"), dict) else {}
        lib_path = lib.get("path") if isinstance(lib.get("path"), str) else "src/lib.rs"
        has_library = "lib" in manifest or posixpath.join(directory, lib_path) in paths
        library = (lib.get("name") or package["name"].replace("-", "_")) if has_library else None
        roots = {"src/lib.rs", "src/main.rs", lib_path}
        tests, stanzas = set(), []
        for kind, key in (("examples", "example"), ("tests", "test"), ("benches", "bench"),
                          ("src/bin", "bin")):
            for target in manifest.get(key, []) or []:
                if not isinstance(target, dict):
                    continue
                target_path = _target_path(directory, kind, target, paths)
                if target_path is None:
                    continue
                roots.add(target_path)
                if key in ("test", "bench") or (key == "example" and target.get("test") is True):
                    tests.add(target_path)
                if key in ("test", "example"):
                    fields = tuple(sorted((k, v) for k, v in target.items()
                                          if k not in ("name", "path")))
                    stanzas.append(Stanza(key, posixpath.join(directory, target_path), fields))
        found.append(Package(directory, package["name"], library, frozenset(roots),
                             frozenset(tests), tuple(stanzas)))
    return found


def package_of(path: str, found: list[Package]) -> Package | None:
    best = None
    for package in found:
        if package.directory == "" or path.startswith(package.directory + "/"):
            if best is None or len(package.directory) > len(best.directory):
                best = package
    return best


# --- reading items -----------------------------------------------------------


@dataclass(frozen=True)
class Owner:
    """The `impl` or `trait` block a method is in."""

    attributes: str  # the block's attributes, as source
    header: str  # its source from after its attributes to before its `{`
    trait: str | None  # a trait declaration's name; None for an `impl`


@dataclass(frozen=True)
class Item:
    file: str
    module: str  # the inline modules around it in its file, `a::b`, or ""
    kind: str  # one of KINDS
    # A method's is `<Type>::<name>`, from its owner, once `read_side` names it.
    name: str
    line: int  # the line of its name
    # The tests it defines: 1 for a fn with a test attribute, and for a macro or a
    # macro invocation each test attribute inside it.
    tests: int
    text: str  # its source, from its first attribute through its end
    owner: Owner | None = None  # a method's or an associated item's `impl` or `trait`
    # The attributes that gate it, each as source: once `read_side` adds them, those
    # of each `mod x;` declaring its file; then the file's inner `cfg`s, and each
    # enclosing inline module's attributes and inner `cfg`s.
    gate: tuple[str, ...] = ()

    @property
    def key(self) -> tuple[str, str, str, str]:
        return (self.file, self.module, self.kind, self.name)

    @property
    def label(self) -> str:
        return f"{self.module}::{self.name}" if self.module else self.name

    @property
    def entry_key(self) -> tuple[str, str, str]:
        """How a `[[drift]]` or `[[retired]]` entry names it."""
        return (self.file, self.kind, self.label)

    @property
    def where(self) -> str:
        place = f"{self.file}:{self.line}" if self.line else self.file
        return f"{place} {self.kind} {self.label}"


@dataclass(frozen=True)
class ModDecl:
    """A `mod x;`, whose body is another file."""

    name: str
    inline: tuple[str, ...]  # the inline modules it is declared in
    path: str | None  # its `#[path = ...]`
    test: bool  # declared in a test context, or under a `cfg` naming `test`
    # Its attributes as source, but its `#[path]`, which only says where the file is.
    attributes: tuple[str, ...] = ()


class Attributes(NamedTuple):
    head: int  # where the item starts, after any inner attribute
    end: int  # where its attributes end
    # Each outer and each inner attribute: the index of its `#`, of its content and of
    # its `]`.
    outer: list[tuple[int, int, int]]
    inner: list[tuple[int, int, int]]


@dataclass
class ParsedFile:
    path: str
    # Every module-level item, and whether it is in a test context by itself.
    items: list[tuple[Item, bool]] = field(default_factory=list)
    decls: list[ModDecl] = field(default_factory=list)


def cfg_names_test(attribute: str) -> bool:
    """Whether a `cfg(...)` attribute's predicate names `test` outside any `not(...)`.

    `attribute` is the text between `#[` and `]` with string contents blanked.
    """
    if not CFG_ATTRIBUTE.match(attribute):
        return False
    stack: list[str] = []
    tokens = re.findall(r"\w+|[()]|=|,", attribute)
    for i, token in enumerate(tokens):
        following = tokens[i + 1] if i + 1 < len(tokens) else ""
        if following == "(":
            stack.append(token)
        elif token == ")":
            if stack:
                stack.pop()
        elif token == "test" and following not in ("(", "="):
            if stack.count("not") % 2 == 0:
                return True
    return False


class Scanner:
    """The module-level items of one Rust file."""

    def __init__(self, path: str, source: str):
        self.path = path
        kinds = lex(source)
        self.source = source
        self.code = view(source, kinds, CODE)
        self.text = view(source, kinds, CODE + STRING)
        self.matches = self._match()
        self.result = ParsedFile(path)
        self._ordinals: dict[tuple[str, str, str], int] = defaultdict(int)

    def _fail(self, at: int, message: str) -> BodyDiffError:
        line = self.code.count("\n", 0, at) + 1
        return BodyDiffError(f"{self.path}:{line}: {message}")

    def _match(self) -> dict[int, int]:
        pairs, stack = {}, []
        closing = {")": "(", "]": "[", "}": "{"}
        for i, c in enumerate(self.code):
            if c in "([{":
                stack.append(i)
            elif c in closing:
                if not stack or self.code[stack[-1]] != closing[c]:
                    raise self._fail(i, f"`{c}` closes nothing open; the file does not balance")
                pairs[stack.pop()] = i
        if stack:
            raise self._fail(stack[-1], f"`{self.code[stack[-1]]}` never closes")
        return pairs

    def _space(self, i: int, end: int) -> int:
        while i < end and self.code[i].isspace():
            i += 1
        return i

    def _word(self, i: int) -> tuple[str, int]:
        match = WORD.match(self.code, i)
        return (match[0], match.end()) if match else ("", i)

    def _semicolon(self, i: int, end: int) -> int:
        """The index after the `;` that ends an item, jumping over every group."""
        while i < end:
            c = self.code[i]
            if c in "([{":
                i = self.matches[i] + 1
            elif c == ";":
                return i + 1
            else:
                i += 1
        raise self._fail(i, "an item never ends with `;`")

    def _block(self, i: int, end: int) -> int:
        """The index after an item that ends at `;` or after its first `{...}`."""
        while i < end:
            c = self.code[i]
            if c in "([":
                i = self.matches[i] + 1
            elif c == "{":
                return self.matches[i] + 1
            elif c == ";":
                return i + 1
            else:
                i += 1
        raise self._fail(i, "an item never ends")

    def _group_then_semicolon(self, i: int, end: int) -> int:
        """After a macro's `{...}`, or its `(...)` or `[...]` and the `;` after it."""
        i = self._space(i, end)
        if i >= end or self.code[i] not in "([{":
            raise self._fail(i, "a macro is followed by no group")
        close = self.matches[i]
        if self.code[i] == "{":
            return close + 1
        after = self._space(close + 1, end)
        return after + 1 if after < end and self.code[after] == ";" else close + 1

    def scan(self) -> ParsedFile:
        self._module(0, len(self.code), (), False, ())
        return self.result

    def _emit(self, kind: str, name: str, name_at: int, span: tuple[int, int],
              attributes: list[str], inline: tuple[str, ...], context: bool,
              gate: tuple[str, ...], owner: Owner | None = None) -> None:
        module = "::".join(inline)
        if kind == "invocation":
            name += "!"
        if (kind == "const" and name == "_") or kind == "invocation" or (
                kind == "extern" and name == "block"):
            self._ordinals[(module, kind, name)] += 1
            name = f"{name}#{self._ordinals[(module, kind, name)]}"
        if kind == "fn":
            tests = int(any(TEST_ATTRIBUTE.match(a) for a in attributes))
        elif kind in ("macro", "invocation"):
            tests = len(NESTED_TEST.findall(self.code, span[0], span[1]))
        else:
            tests = 0
        own = tests > 0 or any(cfg_names_test(a) for a in attributes)
        item = Item(self.path, module, kind, name, self.code.count("\n", 0, name_at) + 1,
                    tests, self.source[span[0]:span[1]], owner, gate)
        self.result.items.append((item, context or own))

    def _attributes(self, i: int, end: int) -> Attributes:
        """The attributes at `i`. An inner attribute starts no item."""
        head, outer, inner = i, [], []
        while i < end and self.code[i] == "#":
            j = self._space(i + 1, end)
            is_inner = j < end and self.code[j] == "!"
            if is_inner:
                j = self._space(j + 1, end)
            if j >= end or self.code[j] != "[":
                raise self._fail(i, "`#` opens no attribute")
            close = self.matches[j]
            if is_inner:
                inner.append((i, j + 1, close))
                head = self._space(close + 1, end)
            else:
                outer.append((i, j + 1, close))
            i = self._space(close + 1, end)
        return Attributes(head, i, outer, inner)

    def _keyword(self, i: int, end: int) -> tuple[str, int]:
        """The keyword after an item's visibility and qualifiers, and where it ends.

        `const` and `extern` qualify a `fn` only when one follows them; otherwise they
        are the item's own keyword (`const X`, `extern crate`, `extern "C" {...}`).
        """
        code = self.code
        word, j = self._word(i)
        if word == "pub":
            j = self._space(j, end)
            if j < end and code[j] == "(":
                j = self.matches[j] + 1
            word, j = self._word(self._space(j, end))
        while word in ("async", "unsafe", "safe", "default", "const", "extern"):
            k = self._space(j, end)
            if word == "extern" and k < end and code[k] == '"':
                k = self._space(code.index('"', k + 1) + 1, end)
            following, after = self._word(k)
            if word in ("const", "extern") and following not in ("fn", "async", "unsafe",
                                                                  "extern"):
                break
            if not following:
                break
            word, j = following, after
        return word, j

    def _fn(self, j: int, end: int) -> tuple[str, int, int]:
        """A `fn`'s name, where its name is, and the index after its body or `;`.
        `j` is just after the keyword."""
        name, k = self._word(self._space(j, end))
        if not name:
            raise self._fail(j, "a `fn` has no name")
        stop = k
        while stop < end:
            c = self.code[stop]
            if c in "([":
                stop = self.matches[stop] + 1
            elif c in "{;":
                return name, k - len(name), (self.matches[stop] if c == "{" else stop) + 1
            else:
                stop += 1
        return name, k - len(name), stop

    def _owner(self, word: str, attributes: Attributes, i: int, j: int, end: int,
               inline: tuple[str, ...], context: bool, gate: tuple[str, ...]) -> int:
        """An `impl` or `trait` block at `i`, whose keyword ends at `j`: each fn, const
        and type in it as a method or an associated item. The index after it."""
        code = self.code
        k = j
        while k < end and code[k] not in "{;":
            k = self.matches[k] + 1 if code[k] in "([" else k + 1
        if k >= end or code[k] == ";":
            return k + 1  # a trait alias, `trait A = B;`
        trait = self._word(self._space(j, end))[0] if word == "trait" else None
        owner = Owner(self.source[attributes.head:i], self.source[i:k], trait)
        contents = [code[a:b] for _, a, b in attributes.outer]
        self._members(k + 1, self.matches[k], owner, inline,
                      context or any(cfg_names_test(a) for a in contents), gate)
        return self.matches[k] + 1

    def _members(self, start: int, end: int, owner: Owner, inline: tuple[str, ...],
                 context: bool, gate: tuple[str, ...]) -> None:
        """The fns, consts and types of an `impl` or `trait` body."""
        code = self.code
        i = start
        while True:
            i = self._space(i, end)
            if i >= end:
                return
            attributes = self._attributes(i, end)
            head, i = attributes.head, attributes.end
            if i >= end:
                return
            if code[i] == ";":
                i += 1
                continue
            contents = [code[a:b] for _, a, b in attributes.outer]
            word, j = self._keyword(i, end)
            if word == "fn":
                name, name_at, stop = self._fn(j, end)
                self._emit("method", name, name_at, (head, stop), contents, inline, context,
                           gate, owner)
                i = stop
            elif word in ("const", "type"):
                k = self._space(j, end)
                name, after = self._word(k)
                stop = self._semicolon(after, end)
                self._emit(f"assoc_{word}", name, k, (head, stop), contents, inline, context,
                           gate, owner)
                i = stop
            else:
                k = self._space(j, end)
                while code.startswith("::", k):
                    k = self._space(self._word(self._space(k + 2, end))[1], end)
                if k < end and code[k] == "!":
                    i = self._group_then_semicolon(k + 1, end)
                else:
                    i = self._block(j, end)

    def _module(self, start: int, end: int, inline: tuple[str, ...], context: bool,
                gate: tuple[str, ...]) -> None:
        code = self.code
        i = start
        while True:
            i = self._space(i, end)
            if i >= end:
                return
            attributes = self._attributes(i, end)
            head, i = attributes.head, attributes.end
            for h, a, b in attributes.inner:
                # An inner `cfg` gates the rest of its module, and one naming `test`
                # makes it a test context.
                if CFG_GATE.match(code[a:b]):
                    gate += (self.source[h:b + 1],)
                    context = context or cfg_names_test(code[a:b])
            if i >= end:
                return
            if code[i] == ";":
                i += 1
                continue
            attribute_texts = tuple(self.source[h:b + 1] for h, _, b in attributes.outer)
            contents = [code[a:b] for _, a, b in attributes.outer]
            word, j = self._keyword(i, end)
            if word == "fn":
                name, name_at, stop = self._fn(j, end)
                self._emit("fn", name, name_at, (head, stop), contents, inline, context, gate)
                i = stop
            elif word in ("const", "static", "type"):
                k = self._space(j, end)
                name, after = self._word(k)
                if word == "static" and name == "mut":
                    k = self._space(after, end)
                    name, after = self._word(k)
                if not name:
                    raise self._fail(j, f"a `{word}` has no name")
                stop = self._semicolon(after, end)
                self._emit(word, name, k, (head, stop), contents, inline, context, gate)
                i = stop
            elif word in ("struct", "enum", "union"):
                k = self._space(j, end)
                name, _ = self._word(k)
                stop = self._block(j, end)
                self._emit(word, name, k, (head, stop), contents, inline, context, gate)
                i = stop
            elif word == "extern":
                k = self._space(j, end)
                following, after = self._word(k)
                name, name_at = "block", i
                if following == "crate":
                    name_at = self._space(after, end)
                    name = self._word(name_at)[0]
                stop = self._block(j, end)
                self._emit("extern", name, name_at, (head, stop), contents, inline, context,
                           gate)
                i = stop
            elif word == "macro_rules":
                k = self._space(j, end)
                if k >= end or code[k] != "!":
                    raise self._fail(k, "`macro_rules` is not followed by `!`")
                k = self._space(k + 1, end)
                name, after = self._word(k)
                stop = self._group_then_semicolon(after, end)
                self._emit("macro", name, k, (head, stop), contents, inline, context, gate)
                i = stop
            elif word == "mod":
                name, after = self._word(self._space(j, end))
                after = self._space(after, end)
                if not name or after >= end or code[after] not in ";{":
                    raise self._fail(j, "a `mod` has no name, or no `;` or body after it")
                test = context or any(cfg_names_test(a) for a in contents)
                if code[after] == ";":
                    path, kept = None, []
                    for (h, a, b), text in zip(attributes.outer, attribute_texts):
                        if (match := PATH_ATTRIBUTE.match(self.text[a:b])) is not None:
                            path = match["path"]
                        else:
                            kept.append(text)
                    self.result.decls.append(ModDecl(name, inline, path, test, tuple(kept)))
                    i = after + 1
                else:
                    close = self.matches[after]
                    self._module(after + 1, close, inline + (name,), test,
                                 gate + attribute_texts)
                    i = close + 1
            elif word in ("impl", "trait"):
                i = self._owner(word, attributes, i, j, end, inline, context, gate)
            elif word == "use":
                i = self._semicolon(j, end)
            elif word in ("auto", "macro"):
                i = self._block(j, end)
            elif word:
                # A macro invocation: its path, `!` and a group.
                name, name_at = word, i
                k = self._space(j, end)
                while code.startswith("::", k):
                    name_at = self._space(k + 2, end)
                    name, k = self._word(name_at)
                    k = self._space(k, end)
                if k >= end or code[k] != "!":
                    raise self._fail(i, f"cannot read the item that starts `{word}`")
                stop = self._group_then_semicolon(k + 1, end)
                self._emit("invocation", name, name_at, (head, stop), contents, inline,
                           context, gate)
                i = stop
            else:
                raise self._fail(i, f"cannot read the item that starts "
                                    f"`{code[i:i + 20].strip()}`")


def scan(path: str, source: str) -> ParsedFile:
    return Scanner(path, source).scan()


def _module_file(decl: ModDecl, owner: str, owner_package: Package | None,
                 paths: set[str]) -> str | None:
    """The file a `mod x;` in `owner` names, if the tree has it."""
    directory, base = posixpath.split(owner)
    relative = owner_package.relative(owner) if owner_package else owner
    mod_rs = base in MOD_RS or bool(CRATE_ROOT.match(relative)) or (
        owner_package is not None and relative in owner_package.roots)
    nested = directory if mod_rs else posixpath.join(directory, base[:-3])
    if decl.path is not None:
        within = posixpath.join(nested, *decl.inline) if decl.inline else directory
        candidates = [posixpath.join(within, decl.path)]
    else:
        within = posixpath.join(nested, *decl.inline)
        candidates = [posixpath.join(within, f"{decl.name}.rs"),
                      posixpath.join(within, decl.name, "mod.rs")]
    for candidate in candidates:
        candidate = posixpath.normpath(candidate)
        if not candidate.startswith("../") and candidate in paths:
            return candidate
    return None


# Makes the normaliser for one file of a tree, from its path, its package and the
# identifiers its code names.
Normalisers = Callable[[str, "Package | None", frozenset[str]], "Normaliser"]


@dataclass
class Side:
    """The test-context items of one tree, by key."""

    tree: Tree
    items: dict[tuple[str, str, str, str], Item]
    # Each `.rs` file's package.
    owners: dict[str, Package | None]
    # The identifiers each file's code names, for scoping renames.
    names: dict[str, frozenset[str]]
    make: Normalisers
    _made: dict[str, Normaliser] = field(default_factory=dict)

    def normaliser(self, path: str) -> Normaliser:
        if path not in self._made:
            self._made[path] = self.make(path, self.owners.get(path),
                                         self.names.get(path, frozenset()))
        return self._made[path]


def library_of(path: str, package: Package | None) -> str | None:
    """What `crate::` names in `path`: its library in `src/`, and nothing elsewhere."""
    if package is None or not package.relative(path).startswith("src/"):
        return None
    return package.library


def plain(tree: Tree) -> Normalisers:
    """The normalisers of a tree no row or literal applies to: HEAD's."""
    def make(path: str, package: Package | None, names: frozenset[str]) -> Normaliser:
        return Normaliser(library=library_of(path, package),
                          directory=posixpath.dirname(path), blob=tree.blob)
    return make


def compact(tokens: list[Token]) -> str:
    """Tokens as text, with a space only where two words meet and after a comma."""
    text = ""
    for i, token in enumerate(tokens):
        if i and (tokens[i - 1].kind in ("word", "num", "life") and
                  token.kind in ("word", "num", "life") or _is(tokens[i - 1], ",")):
            text += " "
        text += token.text
    return text


def owner_type(owner: Owner, normaliser: Normaliser) -> str:
    """A method's `<Type>`: a trait's name, an impl's self type, or
    `<impl Trait for Type>`, each normalised as a body is."""
    if owner.trait is not None:
        return owner.trait
    tokens = normaliser.tokens(owner.header)
    start = next(i for i, t in enumerate(tokens) if _is(t, "impl")) + 1
    if start < len(tokens) and _is(tokens[start], "<"):
        depth = 0
        for start in range(start, len(tokens)):
            depth += _is(tokens[start], "<") - _is(tokens[start], ">")
            if depth == 0:
                break
        start += 1
    angle = 0
    split = stop = None
    for i in range(start, len(tokens)):
        angle += _is(tokens[i], "<") - _is(tokens[i], ">")
        if angle == 0 and _is(tokens[i], "where"):
            stop = i
            break
        if (angle == 0 and _is(tokens[i], "for") and split is None
                and not (i + 1 < len(tokens) and _is(tokens[i + 1], "<"))):
            split = i
    body = tokens[start:stop]
    if split is None:
        return compact(body)
    split -= start
    return f"<impl {compact(body[:split])} for {compact(body[split + 1:])}>"


def _name_owned(owned: list[tuple[Item, Normaliser]]) -> list[Item]:
    """Each method and associated item named `<Type>::<name>`, its block's attributes
    and header put around its text. Two of one name in one module are told apart by
    their blocks' attributes, then by `#n` in source order."""
    named = []
    for item, normaliser in owned:
        owner = item.owner
        named.append((replace(item, name=f"{owner_type(owner, normaliser)}::{item.name}",
                              text=f"{owner.attributes}\n{owner.header} {{\n{item.text}\n}}"),
                      compact(normaliser.tokens(owner.attributes))))
    counts = Counter(item.key for item, _ in named)
    relabelled = [replace(item, name=f"{item.name} {attributes}")
                  if counts[item.key] > 1 and attributes else item
                  for item, attributes in named]
    counts = Counter(item.key for item in relabelled)
    seen: dict[tuple[str, str, str, str], int] = defaultdict(int)
    out = []
    for item in sorted(relabelled, key=lambda item: (item.file, item.line)):
        if counts[item.key] > 1:
            seen[item.key] += 1
            item = replace(item, name=f"{item.name}#{seen[item.key]}")
        out.append(item)
    return out


def read_side(tree: Tree, make: Normalisers | None = None,
              gates: Callable[[str, tuple[str, ...]], tuple[str, ...]] | None = None) -> Side:
    """Every test-context item in `tree`, each method and associated item named
    through `make`'s normaliser for its file, with every manifest stanza of a test or
    an example and every fixture. Refuses two other items of one key.

    An item's compared text starts with what gates it: the attributes of each
    `mod x;` that declares its file, then its `gate`, each passed through `gates`
    with the item's file.
    """
    found = packages(tree)
    paths = set(tree.paths())
    rust = [p for p in sorted(paths) if p.endswith(".rs") and package_of(p, found)]
    if isinstance(tree, Baseline):
        tree.preload(rust)
    owners = {p: package_of(p, found) for p in rust}
    parsed: dict[str, ParsedFile] = {}
    names: dict[str, frozenset[str]] = {}
    for path in rust:
        source = tree.read(path)
        scanner = Scanner(path, source)
        parsed[path] = scanner.scan()
        names[path] = frozenset(IDENTIFIER.findall(scanner.code))
    whole = set()
    for path, package in owners.items():
        relative = package.relative(path)
        if relative.startswith("tests/") or relative in package.test_targets:
            whole.add(path)
    declarations: list[tuple[str, ModDecl, str]] = []
    declared: dict[str, set[str]] = defaultdict(set)
    for path, file in parsed.items():
        for decl in file.decls:
            target = _module_file(decl, path, owners[path], paths)
            if target is not None and target in parsed:
                declarations.append((path, decl, target))
                declared[target].update(decl.attributes)
    changed = True
    while changed:
        changed = False
        for path, decl, target in declarations:
            if (decl.test or path in whole) and target not in whole:
                whole.add(target)
                changed = True
    side = Side(tree, {}, owners, names, make or plain(tree))
    chosen = []
    for path, file in parsed.items():
        for item, context in file.items:
            if context or path in whole:
                gate = (*sorted(declared.get(path, ())), *item.gate)
                chosen.append(replace(item, gate=gates(path, gate) if gates else gate))
    owned = [(item, side.normaliser(item.file)) for item in chosen if item.owner]
    items = [replace(item, text="\n".join([*item.gate, item.text]))
             for item in [item for item in chosen if not item.owner] + _name_owned(owned)]
    for package in found:
        for stanza in package.stanzas:
            fields = json.dumps({"crate": package.name, "fields": dict(stanza.fields)},
                                sort_keys=True, default=str)
            items.append(Item(stanza.path, "", "target", f"[[{stanza.key}]]", 0, 0, fields))
    items += [Item(path, "", "fixture", posixpath.basename(path), 0, 0,
                   f"blob {tree.blob(path)}") for path in sorted(paths) if is_fixture(path)]
    for item in items:
        if item.key in side.items:
            raise BodyDiffError(f"{tree.label}: {item.file}:{item.line} has a second "
                                f"{item.kind} {item.label} in its module, so the two "
                                f"cannot be told apart")
        side.items[item.key] = item
    return side


# --- normalising -------------------------------------------------------------


class Token(NamedTuple):
    kind: str  # "word", "punct", "str", "char", "num", "life", "blob"
    text: str
    # A string literal's opening delimiter, content as written, and closing one.
    open: str = ""
    content: str = ""
    close: str = ""


RAW_STRING = re.compile(r'(?:b|c)?r(#*)"')
STRING_OPEN = re.compile(r'(?:b|c)?"')
NUMBER = re.compile(r"\d\w*(?:\.\d\w*)?")
TOKEN_WORD = re.compile(r"(?:r#)?[^\W\d]\w*")
PUNCTUATION = ("..=", "...", "::", "=>", "->", "..")


def tokenize(text: str, label: str = "<text>") -> list[Token]:
    """The tokens of Rust `text`, whose comments are already blanked."""
    tokens: list[Token] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
            continue
        if (match := RAW_STRING.match(text, i)) is not None:
            closing = '"' + match[1]
            close = text.find(closing, match.end())
            if close == -1:
                raise BodyDiffError(f"{label}: a raw string never closes")
            end = close + len(closing)
            tokens.append(Token("str", text[i:end], match[0], text[match.end():close], closing))
            i = end
            continue
        if (match := STRING_OPEN.match(text, i)) is not None:
            j = match.end()
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            if j >= n:
                raise BodyDiffError(f"{label}: a string never closes")
            tokens.append(Token("str", text[i:j + 1], match[0], text[match.end():j], '"'))
            i = j + 1
            continue
        quote = i + 1 if text.startswith("b'", i) else i if c == "'" else -1
        if quote != -1:
            if text.startswith("'\\", quote):
                end = text.find("'", quote + 3) + 1
            elif quote + 2 < n and text[quote + 2] == "'" and text[quote + 1] != "\n":
                end = quote + 3
            else:
                end = 0
            if end > 0:
                tokens.append(Token("char", text[i:end]))
                i = end
                continue
            if (match := TOKEN_WORD.match(text, i + 1)) is not None:
                tokens.append(Token("life", "'" + match[0]))
                i = match.end()
                continue
        if (match := TOKEN_WORD.match(text, i)) is not None:
            tokens.append(Token("word", match[0]))
            i = match.end()
            continue
        if (match := NUMBER.match(text, i)) is not None:
            tokens.append(Token("num", match[0]))
            i = match.end()
            continue
        punct = next((p for p in PUNCTUATION if text.startswith(p, i)), c)
        tokens.append(Token("punct", punct))
        i += len(punct)
    return tokens


def _closer(tokens: list[Token], i: int) -> int:
    """The index of the token closing the bracket at `i`."""
    depth = 0
    for j in range(i, len(tokens)):
        if tokens[j].kind == "punct":
            if tokens[j].text in "([{":
                depth += 1
            elif tokens[j].text in ")]}":
                depth -= 1
                if depth == 0:
                    return j
    raise BodyDiffError("a bracket never closes")


def _opener(tokens: list[Token], i: int) -> int:
    """The index of the token opening the bracket closed at `i`."""
    depth = 0
    for j in range(i, -1, -1):
        if tokens[j].kind == "punct":
            if tokens[j].text in ")]}":
                depth += 1
            elif tokens[j].text in "([{":
                depth -= 1
                if depth == 0:
                    return j
    raise BodyDiffError("a bracket never opens")


def _semicolon(tokens: list[Token], i: int) -> int:
    """The index of the `;` ending the statement that starts at `i`."""
    depth = 0
    for j in range(i, len(tokens)):
        if tokens[j].kind != "punct":
            continue
        if tokens[j].text in "([{":
            depth += 1
        elif tokens[j].text in ")]}":
            depth -= 1
        elif tokens[j].text == ";" and depth == 0:
            return j
    raise BodyDiffError("a `use` never ends with `;`")


def _is(token: Token, text: str) -> bool:
    return token.kind in ("punct", "word") and token.text == text


def _statement_starts(tokens: list[Token], i: int) -> bool:
    """Whether a statement can start at `tokens[i]`: at the start, or after `;`, `{`,
    `}` or an attribute. `impl Trait + use<'a>` holds a `use` that starts none."""
    if i == 0:
        return True
    before = tokens[i - 1]
    if before.kind == "punct" and before.text in (";", "{", "}"):
        return True
    if _is(before, "]"):
        opener = _opener(tokens, i - 1)
        return (opener > 0 and _is(tokens[opener - 1], "#")) or (
            opener > 1 and _is(tokens[opener - 1], "!") and _is(tokens[opener - 2], "#"))
    return False


def drop_comments_and_uses(tokens: list[Token]) -> list[Token]:
    """Step 1: without `#[doc = ...]` attributes and whole `use ...;` statements.

    Comments, doc comments among them, are blanked before tokenizing.
    """
    out: list[Token] = []
    i = 0
    while i < len(tokens):
        token = tokens[i]
        if (_is(token, "#") and i + 2 < len(tokens) and _is(tokens[i + 1], "[")
                and _is(tokens[i + 2], "doc") and i + 3 < len(tokens)
                and _is(tokens[i + 3], "=")):
            i = _closer(tokens, i + 1) + 1
            continue
        if token.kind == "word" and token.text == "use":
            # Its visibility goes with it.
            start = len(out)
            if out and _is(out[-1], "pub"):
                start -= 1
            elif out and _is(out[-1], ")"):
                opener = _opener(out, len(out) - 1)
                if opener > 0 and _is(out[opener - 1], "pub"):
                    start = opener - 1
            if _statement_starts(out, start):
                del out[start:]
                i = _semicolon(tokens, i) + 1
                continue
        out.append(token)
        i += 1
    return out


def rewrite_crate(tokens: list[Token], library: str | None) -> list[Token]:
    """Step 2: `crate::` as `<library>::`."""
    if library is None:
        return tokens
    out = list(tokens)
    for i, token in enumerate(tokens):
        if (token.kind == "word" and token.text == "crate" and i + 1 < len(tokens)
                and _is(tokens[i + 1], "::") and not (i > 0 and _is(tokens[i - 1], "$"))):
            out[i] = Token("word", library)
    return out


def renames_of(rows: Iterable[RowLike], library: str | None,
               names: frozenset[str]) -> dict[str, str]:
    """The identifier renames rows make in one baseline file: each row with a new path
    whose last segment differs, from a crate the file is in or names."""
    return {old: new for old, (new, _) in _renames(rows, library, names).items()}


def _renames(rows: Iterable[RowLike], library: str | None,
             names: frozenset[str]) -> dict[str, tuple[str, str]]:
    """`renames_of`, with the old path of the row each rename comes from."""
    renames: dict[str, tuple[str, RowLike]] = {}
    for row in rows:
        if not row.new:
            continue
        old_last, new_last = row.old.split("::")[-1], row.new.split("::")[-1]
        crate = row.old.split("::")[0]
        if old_last == new_last or not (crate == library or crate in names):
            continue
        if old_last in renames and renames[old_last][0] != new_last:
            other = renames[old_last][1]
            raise BodyDiffError(
                f"the manifest renames `{old_last}` to `{new_last}` (`{row.old}`) and to "
                f"`{renames[old_last][0]}` (`{other.old}`), so a file that names both "
                f"crates cannot be normalised")
        renames[old_last] = (new_last, row)
    return {old: (new, row.old) for old, (new, row) in renames.items()}


def apply_rows(tokens: list[Token], rows: Iterable[RowLike], renames: dict[str, str],
               used: set | None = None, sources: dict[str, str] | None = None) -> list[Token]:
    """Step 3: each path under the row with its longest old path, a later row winning
    on an equal one; and a path no row covers renamed at its head, the one identifier
    not after `::` or `.`, so `Vec::new` keeps its `new` whatever `Client::new` became.

    `used` collects `("path", old)` for each row that rewrote or renamed something,
    `sources` naming the row each rename comes from.
    """
    table = {tuple(row.old.split("::")): row for row in rows}
    out: list[Token] = []
    i = 0
    while i < len(tokens):
        token = tokens[i]
        previous = out[-1] if out else None
        # A `::` leads a path when nothing it could continue comes before it.
        leading = previous is not None and _is(previous, "::") and (
            len(out) < 2 or (out[-2].kind == "punct" and out[-2].text not in (">", ")")))
        starts = token.kind == "word" and (leading or not (
            previous is not None and (_is(previous, ".") or _is(previous, "::")
                                      or _is(previous, "$"))))
        if not starts:
            out.append(token)
            i += 1
            continue
        segments = [token.text]
        j = i + 1
        while (j + 1 < len(tokens) and _is(tokens[j], "::")
               and tokens[j + 1].kind == "word"):
            segments.append(tokens[j + 1].text)
            j += 2
        row = next((table[tuple(segments[:n])] for n in range(len(segments), 0, -1)
                    if tuple(segments[:n]) in table), None)
        path = segments
        if row is not None and row.new:
            path = row.new.split("::") + segments[len(row.old.split("::")):]
            if used is not None:
                used.add(("path", row.old))
        elif row is None and segments[0] in renames:
            path = [renames[segments[0]]] + segments[1:]
            if used is not None and sources:
                used.add(("path", sources[segments[0]]))
        for k, segment in enumerate(path):
            if k:
                out.append(Token("punct", "::"))
            out.append(Token("word", segment))
        i = j
    return out


def hash_includes(tokens: list[Token], directory: str,
                  blob: Callable[[str], str | None]) -> list[Token]:
    """Step 4: an `include_str!` or `include_bytes!` target as its blob's id."""
    out = list(tokens)
    for i, token in enumerate(tokens):
        if not (token.kind == "word" and token.text in ("include_str", "include_bytes")):
            continue
        if not (i + 3 < len(tokens) and _is(tokens[i + 1], "!") and _is(tokens[i + 2], "(")
                and tokens[i + 3].kind == "str"):
            continue
        target = posixpath.normpath(posixpath.join(directory, tokens[i + 3].content))
        sha = None if target.startswith("../") else blob(target)
        out[i + 3] = Token("blob", f"blob:{sha or 'missing'}")
    return out


def replace_literals(tokens: list[Token], literals: dict[str, str], used: set | None = None,
                     scopes: dict[str, str] | None = None) -> list[Token]:
    """Step 5: each string literal whose content `literals` names, as its value.
    `used` collects `("literal", scope, key)` for each one replaced."""
    if not literals:
        return tokens
    out = []
    for token in tokens:
        if token.kind == "str" and token.content in literals:
            if used is not None:
                used.add(("literal", (scopes or {}).get(token.content, ""), token.content))
            content = literals[token.content]
            token = Token("str", token.open + content + token.close, token.open, content,
                          token.close)
        out.append(token)
    return out


def relayout(tokens: list[Token]) -> list[Token]:
    """Step 6: without the layout rustfmt adds when a line re-wraps: the braces around a
    lone expression after `=>` or a closure's `|...|`, and a trailing comma before a
    closing bracket or a generic list's `>`, which only a generic list's end follows."""
    drop: set[int] = set()
    comma_after: set[int] = set()
    stack: list[int] = []
    pairs: dict[int, int] = {}
    for i, token in enumerate(tokens):
        if token.kind != "punct":
            continue
        if token.text in "([{":
            stack.append(i)
        elif token.text in ")]}" and stack:
            pairs[stack.pop()] = i
    for open_at, close_at in pairs.items():
        if not (_is(tokens[open_at], "{") and open_at > 0 and close_at > open_at + 1):
            continue
        before = tokens[open_at - 1]
        if not (_is(before, "=>") or _is(before, "|")):
            continue
        depth, lone = 0, True
        for token in tokens[open_at + 1:close_at]:
            if token.kind == "punct" and token.text in "([{":
                depth += 1
            elif token.kind == "punct" and token.text in ")]}":
                depth -= 1
            elif depth == 0 and _is(token, ";"):
                lone = False
                break
        if not lone:
            continue
        drop |= {open_at, close_at}
        following = tokens[close_at + 1] if close_at + 1 < len(tokens) else None
        if _is(before, "=>") and following is not None and following.text not in (
                ",", ";", "}"):
            comma_after.add(close_at)
    kept: list[Token] = []
    for i, token in enumerate(tokens):
        if i not in drop:
            kept.append(token)
        if i in comma_after:
            kept.append(Token("punct", ","))
    return [token for i, token in enumerate(kept)
            if not (_is(token, ",") and i + 1 < len(kept) and kept[i + 1].kind == "punct"
                    and kept[i + 1].text in ")]}>" and not _one_tuple(kept, i))]


# Words a `(` can follow without being called: `return (x,)` builds a tuple.
NOT_CALLED = frozenset({"return", "in", "if", "match", "while", "let", "mut", "else", "break",
                        "move", "as", "for", "yield", "ref"})


def _one_tuple(tokens: list[Token], comma: int) -> bool:
    """Whether the `,` at `comma` makes a one-element tuple, `(x,)`: it closes a
    `(...)` group holding no other top-level comma that is not a call's arguments."""
    if not _is(tokens[comma + 1], ")"):
        return False
    opener = _opener(tokens, comma + 1)
    depth = 0
    for token in tokens[opener + 1:comma]:
        if token.kind == "punct" and token.text in "([{":
            depth += 1
        elif token.kind == "punct" and token.text in ")]}":
            depth -= 1
        elif depth == 0 and _is(token, ","):
            return False
    if opener + 1 == comma:
        return False
    before = tokens[opener - 1] if opener else None
    called = before is not None and (
        (before.kind == "word" and before.text not in NOT_CALLED)
        or (before.kind == "punct" and before.text in (")", "]", ">", "!", "?")))
    return not called


@dataclass(frozen=True)
class Normaliser:
    """How to normalise the items of one file."""

    library: str | None = None  # what `crate::` names, or None to keep it
    rows: tuple[RowLike, ...] = ()  # the baseline side's only
    renames: dict[str, str] = field(default_factory=dict)  # the baseline side's only
    literals: dict[str, str] = field(default_factory=dict)  # the baseline side's only
    directory: str = ""  # the file's directory, which an include path starts from
    blob: Callable[[str], str | None] = lambda path: None
    # The old path of the row each rename comes from, and the scope each literal is
    # from, for `used`, which collects what replaced something.
    sources: dict[str, str] = field(default_factory=dict)
    scopes: dict[str, str] = field(default_factory=dict)
    used: set | None = None

    def tokens(self, text: str, label: str = "<text>") -> list[Token]:
        text = view(text, lex(text), CODE + STRING)
        tokens = drop_comments_and_uses(tokenize(text, label))
        tokens = rewrite_crate(tokens, self.library)
        if self.rows or self.renames:
            tokens = apply_rows(tokens, self.rows, self.renames, self.used, self.sources)
        tokens = hash_includes(tokens, self.directory, self.blob)
        tokens = replace_literals(tokens, self.literals, self.used, self.scopes)
        return relayout(tokens)


def normalise(text: str, normaliser: Normaliser, label: str = "<text>") -> list[str]:
    """`text`'s tokens, normalised, as lines broken after `;`, `{` and `}`."""
    lines: list[str] = []
    line: list[str] = []
    for token in normaliser.tokens(text, label):
        if token.text == "}" and token.kind == "punct" and line:
            lines.append(" ".join(line))
            line = []
        line.append(token.text)
        if token.kind == "punct" and token.text in ";{}":
            lines.append(" ".join(line))
            line = []
    if line:
        lines.append(" ".join(line))
    return lines


def diff_lines(old: list[str], new: list[str], old_label: str, new_label: str) -> list[str]:
    return list(difflib.unified_diff(old, new, old_label, new_label, lineterm=""))


def diff_sha256(lines: list[str]) -> str:
    """The sha256 of a unified diff's hunks, its two file lines left out."""
    return hashlib.sha256("\n".join(lines[2:]).encode("utf-8")).hexdigest()


# --- ci/test-body-diff.toml --------------------------------------------------


@dataclass(frozen=True)
class Entry:
    """A `[[drift]]` or `[[retired]]` entry, naming a baseline item."""

    file: str
    item: str  # its label: its module path and name, `tests::name`
    kind: str
    row: str
    sha256: str = ""  # `[[drift]]` only

    @property
    def key(self) -> tuple[str, str, str]:
        """The `Item.entry_key` of the item it names."""
        return (self.file, self.kind, self.item)

    @property
    def where(self) -> str:
        return f"{self.file} {self.kind} {self.item}"


@dataclass(frozen=True)
class PathRow:
    """A `[paths]` entry, applied as a manifest path row is."""

    old: str
    new: str
    reason: None = None


def in_scope(path: str, scope: str) -> bool:
    """Whether `path` is `scope` or under it, at a `/`. Every path is under ""."""
    return not scope or path == scope or path.startswith(scope.rstrip("/") + "/")


@dataclass(frozen=True)
class Config:
    files: dict[str, str] = field(default_factory=dict)
    # Each scope, a baseline path prefix or "" for every file, to its literals.
    literals: dict[str, dict[str, str]] = field(default_factory=dict)
    drift: tuple[Entry, ...] = ()
    retired: tuple[Entry, ...] = ()
    # Old paths to new ones the manifest does not hold, such as a private module's.
    paths: dict[str, str] = field(default_factory=dict)
    # Each HEAD path prefix, to the module features, or the whole cfg predicates,
    # whose gates it drops there.
    gates: dict[str, tuple[str, ...]] = field(default_factory=dict)

    def literal_scopes_for(self, path: str) -> dict[str, str]:
        """Each literal key that applies in `path`, to the scope it comes from: the
        longest that holds it."""
        scopes: dict[str, str] = {}
        for scope in sorted(self.literals, key=len):
            if in_scope(path, scope):
                scopes.update(dict.fromkeys(self.literals[scope], scope))
        return scopes

    def literals_for(self, path: str) -> dict[str, str]:
        return {key: self.literals[scope][key]
                for key, scope in self.literal_scopes_for(path).items()}


def _strings(where: str, table) -> dict[str, str]:
    if not isinstance(table, dict) or not all(isinstance(v, str) for v in table.values()):
        raise BodyDiffError(f"{CONFIG}: {where} must be a table mapping strings to strings")
    return dict(table)


def _entries(data: dict, name: str, fields: tuple[str, ...]) -> tuple[Entry, ...]:
    entries = data.get(name, [])
    if not isinstance(entries, list):
        raise BodyDiffError(f"{CONFIG}: `{name}` must be an array of tables, `[[{name}]]`")
    out = []
    for n, entry in enumerate(entries, 1):
        where = f"`[[{name}]]` entry {n}"
        if not isinstance(entry, dict):
            raise BodyDiffError(f"{CONFIG}: {where} is not a table")
        unknown = sorted(set(entry) - set(fields) - {"kind"})
        missing = [f for f in fields if not isinstance(entry.get(f), str) or not entry[f]]
        if unknown or missing:
            raise BodyDiffError(f"{CONFIG}: {where} must give {', '.join(fields)} as strings, "
                                f"and `kind` optionally; it lacks {missing or 'nothing'} and "
                                f"has unknown {unknown or 'nothing'}")
        kind = entry.get("kind", "fn")
        if kind not in KINDS:
            raise BodyDiffError(f"{CONFIG}: {where} has kind {kind!r}, not one of {KINDS}")
        if name == "drift" and not SHA256.match(entry["sha256"]):
            raise BodyDiffError(f"{CONFIG}: {where} has sha256 {entry['sha256']!r}, which is "
                                f"not 64 lowercase hex digits")
        found = Entry(entry["file"], entry["item"], kind, entry["row"], entry.get("sha256", ""))
        if any(other.key == found.key for other in out):
            raise BodyDiffError(f"{CONFIG}: {where} names {found.where}, which an earlier "
                                f"`[[{name}]]` entry names too")
        out.append(found)
    return tuple(out)


def parse_config(text: str) -> Config:
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as err:
        raise BodyDiffError(f"{CONFIG} is not valid TOML: {err}") from err
    unknown = sorted(set(data) - {"files", "literals", "paths", "gates", "drift", "retired"})
    if unknown:
        raise BodyDiffError(f"{CONFIG} has unknown keys {unknown}")
    table = data.get("literals", {})
    if not isinstance(table, dict):
        raise BodyDiffError(f"{CONFIG}: `[literals]` must be a table")
    literals: dict[str, dict[str, str]] = {}
    for key, value in table.items():
        if isinstance(value, str):
            literals.setdefault("", {})[key] = value
        else:
            literals[key] = _strings(f"`[literals.{key!r}]`", value)
    table = data.get("gates", {})
    if not isinstance(table, dict):
        raise BodyDiffError(f"{CONFIG}: `[gates]` must be a table")
    gates = {}
    for prefix, value in table.items():
        features = [value] if isinstance(value, str) else value
        if (not isinstance(features, list) or not features
                or not all(isinstance(f, str) and f.strip() for f in features)):
            raise BodyDiffError(f"{CONFIG}: `[gates]` maps {prefix!r} to {value!r}; give a "
                                f"feature or a cfg predicate, or an array of them")
        for gate in features:
            if is_predicate(gate) and not _balanced(gate):
                raise BodyDiffError(f"{CONFIG}: `[gates]` maps {prefix!r} to the predicate "
                                    f"{gate!r}, which does not read as a cfg predicate: give "
                                    f"what goes inside `cfg(...)`, such as "
                                    f"'any(feature = \"a\", feature = \"b\")'")
        gates[prefix] = tuple(features)
    return Config(_strings("`[files]`", data.get("files", {})), literals,
                  _entries(data, "drift", ("file", "item", "row", "sha256")),
                  _entries(data, "retired", ("file", "item", "row")),
                  _strings("`[paths]`", data.get("paths", {})), gates)


def _balanced(predicate: str) -> bool:
    """Whether `predicate` is one cfg predicate: it starts with a word and its
    brackets balance."""
    try:
        tokens = _predicate_tokens(predicate)
    except BodyDiffError:
        return False
    depth = 0
    for token in tokens:
        if token.kind == "punct" and token.text in "([{":
            depth += 1
        elif token.kind == "punct" and token.text in ")]}":
            depth -= 1
            if depth < 0:
                return False
    return bool(tokens) and tokens[0].kind == "word" and depth == 0


def read_config(root: Path) -> Config:
    try:
        return parse_config((root / CONFIG).read_text(encoding="utf-8"))
    except OSError as err:
        raise BodyDiffError(f"could not read {CONFIG}: {err}") from err


def drift_rows(text: str) -> frozenset[str]:
    """The DRIFT row ids of divergences.md, `R1` and on."""
    return frozenset(DRIFT_ROW.findall(text))


# --- the comparison ----------------------------------------------------------


@dataclass(frozen=True)
class Diff:
    old: Item
    new: Item
    lines: tuple[str, ...]
    sha256: str


@dataclass
class Result:
    baseline: str
    old: dict[tuple[str, str, str, str], Item]
    new: dict[tuple[str, str, str, str], Item]
    file_pairs: dict[str, str | None]
    pairs: list[tuple[Item, Item]] = field(default_factory=list)
    diffs: list[Diff] = field(default_factory=list)
    acknowledged: list[tuple[Diff, Entry]] = field(default_factory=list)
    lost: list[Item] = field(default_factory=list)
    retired: list[tuple[Item, Entry]] = field(default_factory=list)
    problems: list[str] = field(default_factory=list)

    @property
    def added(self) -> list[Item]:
        paired = {new.key for _, new in self.pairs}
        return [item for key, item in sorted(self.new.items()) if key not in paired]

    @property
    def status(self) -> int:
        return 1 if self.diffs or self.lost or self.problems else 0


def pair_items(old: dict, new: dict, file_pairs: dict[str, str | None]) -> list[tuple[Item, Item]]:
    """Each baseline item with its counterpart at HEAD: the item of its key in its
    file's pair, or else the one item anywhere with its `(module path, kind, name)`."""
    pairs, claimed, left = [], set(), []
    for key, item in sorted(old.items()):
        target = file_pairs.get(item.file)
        counterpart = new.get((target, item.module, item.kind, item.name)) if target else None
        if counterpart is not None and counterpart.key not in claimed:
            pairs.append((item, counterpart))
            claimed.add(counterpart.key)
        else:
            left.append(item)
    by_name: dict[tuple[str, str, str], list[Item]] = defaultdict(list)
    for key, item in new.items():
        if key not in claimed:
            by_name[key[1:]].append(item)
    wanted: dict[tuple[str, str, str], int] = defaultdict(int)
    for item in left:
        wanted[item.key[1:]] += 1
    for item in left:
        candidates = by_name.get(item.key[1:], [])
        if len(candidates) == 1 and wanted[item.key[1:]] == 1:
            pairs.append((item, candidates[0]))
    return pairs


def is_predicate(gate: str) -> bool:
    """Whether a `[gates]` value is a whole cfg predicate, such as
    `any(feature = "a", feature = "b")`, rather than a feature's name."""
    return "(" in gate or "=" in gate


def _predicate_tokens(text: str) -> list[Token]:
    return tokenize(view(text, lex(text), CODE + STRING))


def drop_feature_gate(attribute: str, feature: str) -> str | None:
    """`attribute` without the gate on `feature`: None when it is exactly
    `cfg(feature = "<feature>")`, and without that operand when it is a `cfg(all(...))`
    holding it, a lone operand left unwrapped. Any other attribute as it was.

    `feature` may instead be a whole predicate (`is_predicate`), which is dropped the
    same way: where it is the attribute's whole `cfg(...)`, or an operand of its
    top-level `all(...)`, compared token by token."""
    wanted = [t.text for t in _predicate_tokens(feature)] if is_predicate(feature) else None
    tokens = tokenize(view(attribute, lex(attribute), CODE + STRING))
    start = 2 if len(tokens) > 2 and _is(tokens[1], "!") else 1
    if not (len(tokens) > start + 3 and _is(tokens[0], "#") and _is(tokens[start], "[")
            and _is(tokens[start + 1], "cfg") and _is(tokens[start + 2], "(")
            and _closer(tokens, start + 2) == len(tokens) - 2 and _is(tokens[-1], "]")):
        return attribute
    predicate = tokens[start + 3:-2]

    def is_gate(operand: list[Token]) -> bool:
        if wanted is not None:
            return [t.text for t in operand] == wanted
        return (len(operand) == 3 and _is(operand[0], "feature") and _is(operand[1], "=")
                and operand[2].kind == "str" and operand[2].content == feature)

    if is_gate(predicate):
        return None
    if not (len(predicate) > 2 and _is(predicate[0], "all") and _is(predicate[1], "(")
            and _closer(predicate, 1) == len(predicate) - 1):
        return attribute
    operands, depth, current = [], 0, []
    for token in predicate[2:-1]:
        if token.kind == "punct" and token.text in "([{":
            depth += 1
        elif token.kind == "punct" and token.text in ")]}":
            depth -= 1
        if depth == 0 and _is(token, ","):
            operands.append(current)
            current = []
        else:
            current.append(token)
    if current:
        operands.append(current)
    kept = [operand for operand in operands if not is_gate(operand)]
    if len(kept) == len(operands):
        return attribute
    if not kept:
        return None
    inner = " , ".join(" ".join(t.text for t in operand) for operand in kept)
    if len(kept) > 1:
        inner = f"all ( {inner} )"
    return f"{' '.join(t.text for t in tokens[:start + 3])} {inner} ) ]"


def map_feature(crate: str, feature: str, rows: Iterable[RowLike]) -> str:
    """`feature` of baseline package `crate` under the manifest's `## Features` row
    with the longest old `<crate>/<feature>` or `<crate>` covering it."""
    key = f"{crate}/{feature}"
    covering = [row for row in rows if key == row.old or key.startswith(row.old + "/")]
    row = max(covering, key=lambda row: len(row.old), default=None)
    if row is None or not row.new:
        return feature
    mapped = row.new + key[len(row.old):]
    return mapped.split("/", 1)[1] if "/" in mapped else feature


def stanza_lines(text: str, features: Iterable[RowLike] | None = None,
                 drop: Callable[[str], bool] | None = None) -> list[str]:
    """A target stanza's fields as `key = value` lines, its `required-features` sorted
    and, on the baseline side, mapped through the `## Features` rows; at HEAD, without
    each feature `drop` says its `[gates]` entry drops."""
    data = json.loads(text)
    fields = data["fields"]
    required = fields.get("required-features")
    if isinstance(required, list):
        if features is not None:
            required = [map_feature(data["crate"], f, features) if isinstance(f, str) else f
                        for f in required]
        if drop is not None:
            required = [f for f in required if not (isinstance(f, str) and drop(f))]
        # None required is the same as no list.
        if required:
            fields["required-features"] = sorted(required, key=str)
        else:
            del fields["required-features"]
    return [f"{key} = {json.dumps(value, sort_keys=True, default=str)}"
            for key, value in sorted(fields.items())]


def compare(root: Path, baseline: str, rows: tuple[RowLike, ...], config: Config,
            valid_drift: frozenset[str], old_tree: Tree | None = None,
            new_tree: Tree | None = None, features: tuple[RowLike, ...] = ()) -> Result:
    old_tree = old_tree or Baseline(root, baseline)
    new_tree = new_tree or Checkout(root)
    # `[paths]` applies as the manifest's path rows do, after them, so it wins on an
    # equal old path.
    path_rows = tuple(row for row in rows if row.old not in config.paths) + tuple(
        PathRow(old, new) for old, new in config.paths.items())
    used: set = set()

    def baseline_normaliser(path: str, package: Package | None,
                            names: frozenset[str]) -> Normaliser:
        renames = _renames(path_rows, package.library if package else None, names)
        return Normaliser(library=library_of(path, package), rows=path_rows,
                          renames={old: new for old, (new, _) in renames.items()},
                          literals=config.literals_for(path),
                          directory=posixpath.dirname(path), blob=old_tree.blob,
                          sources={old: source for old, (_, source) in renames.items()},
                          scopes=config.literal_scopes_for(path), used=used)

    def gates_for(path: str) -> list[tuple[str, str]]:
        return [(prefix, feature) for prefix, features in sorted(config.gates.items())
                if in_scope(path, prefix) for feature in features]

    def drop_gates(path: str, gate: tuple[str, ...]) -> tuple[str, ...]:
        """HEAD's `gate` for an item in `path`, without what `[gates]` drops there."""
        out = []
        for attribute in gate:
            for prefix, feature in gates_for(path):
                if attribute is None:
                    break
                kept = drop_feature_gate(attribute, feature)
                if kept != attribute:
                    used.add(("gate", prefix, feature))
                attribute = kept
            if attribute is not None:
                out.append(attribute)
        return tuple(out)

    def drop_required(path: str) -> Callable[[str], bool]:
        def drop(feature: str) -> bool:
            for prefix, gated in gates_for(path):
                if gated == feature:
                    used.add(("gate", prefix, feature))
                    return True
            return False
        return drop

    old_side = read_side(old_tree, baseline_normaliser)
    new_side = read_side(new_tree, gates=drop_gates if config.gates else None)
    old_paths, new_paths = set(old_tree.paths()), set(new_tree.paths())
    result = Result(baseline, old_side.items, new_side.items, {})

    renamed = file_pairs(root, baseline)
    holding = {item.file for item in old_side.items.values()}
    for old_file, new_file in sorted(config.files.items()):
        if old_file not in old_paths or new_file not in new_paths:
            result.problems.append(
                f"`[files]` pairs {old_file} with {new_file}, but "
                f"{old_file if old_file not in old_paths else new_file} is not in "
                f"{baseline if old_file not in old_paths else 'HEAD'}")
        elif old_file not in holding:
            result.problems.append(f"`[files]` pairs {old_file} with {new_file}, but "
                                   f"{old_file} holds no item to compare")
    for path in sorted(holding):
        if path in config.files:
            result.file_pairs[path] = config.files[path]
        elif path in renamed:
            result.file_pairs[path] = renamed[path]
        else:
            result.file_pairs[path] = path if path in new_paths else None

    result.pairs = pair_items(old_side.items, new_side.items, result.file_pairs)
    drift = {entry.key: entry for entry in config.drift}
    used_drift: set[tuple[str, str, str]] = set()
    for old, new in result.pairs:
        if old.kind == "target":
            before = stanza_lines(old.text, features)
            after = stanza_lines(new.text, drop=drop_required(new.file))
        elif old.kind == "fixture":
            before, after = [old.text], [new.text]
        else:
            before = normalise(old.text, old_side.normaliser(old.file), old.file)
            after = normalise(new.text, new_side.normaliser(new.file), new.file)
        if before == after:
            continue
        lines = diff_lines(before, after, f"{baseline}:{old.where}", f"HEAD:{new.where}")
        found = Diff(old, new, tuple(lines), diff_sha256(lines))
        entry = drift.get(old.entry_key)
        if entry is not None and entry.sha256 == found.sha256 and entry.row in valid_drift:
            result.acknowledged.append((found, entry))
            used_drift.add(old.entry_key)
        else:
            result.diffs.append(found)
    for scope, table in sorted(config.literals.items()):
        for key in table:
            if ("literal", scope, key) not in used:
                where = f"`[literals.{scope!r}]`" if scope else "`[literals]`"
                result.problems.append(f"{where} replaces {key!r}, but no baseline literal "
                                       f"it applies to holds it")
    for prefix, features in sorted(config.gates.items()):
        for feature in features:
            if ("gate", prefix, feature) not in used:
                what = (f"`cfg({feature})`" if is_predicate(feature)
                        else f"the `{feature}` gate")
                result.problems.append(
                    f"`[gates]` drops {what} under {prefix}, but no item or "
                    f"stanza at HEAD there has it")
    for old_path in sorted(config.paths):
        if ("path", old_path) not in used:
            result.problems.append(f"`[paths]` maps `{old_path}`, but no baseline path or "
                                   f"identifier it covers was rewritten")
    for entry in config.drift:
        if entry.row not in valid_drift:
            result.problems.append(f"`[[drift]]` for {entry.where} names {entry.row}, which "
                                   f"is not a DRIFT row of {DIVERGENCES}")
        elif entry.key not in used_drift:
            result.problems.append(f"`[[drift]]` for {entry.where} is stale: that item has no "
                                   f"diff whose sha256 is {entry.sha256}")

    paired = {old.key for old, _ in result.pairs}
    retired = {entry.key: entry for entry in config.retired}
    lost_keys = set()
    for key, item in sorted(old_side.items.items()):
        if key in paired:
            continue
        lost_keys.add(item.entry_key)
        entry = retired.get(item.entry_key)
        if entry is None:
            result.lost.append(item)
            continue
        row = _longest_row(rows, entry.row)
        if row is None:
            result.problems.append(f"`[[retired]]` for {entry.where} cites `{entry.row}`, "
                                   f"which no manifest row covers")
            result.lost.append(item)
        elif row.new:
            result.problems.append(f"`[[retired]]` for {entry.where} cites `{entry.row}`, "
                                   f"which the manifest row `{row.old}` maps to `{row.new}`; "
                                   f"a retired test's subject must be gone, with a reason")
            result.lost.append(item)
        else:
            result.retired.append((item, entry))
    for entry in config.retired:
        if entry.key not in lost_keys:
            result.problems.append(f"`[[retired]]` for {entry.where} is stale: that item is "
                                   f"not a lost baseline item")
    return result


def _longest_row(rows: Iterable[RowLike], path: str) -> RowLike | None:
    covering = [row for row in rows if path == row.old or path.startswith(row.old + "::")]
    return max(covering, key=lambda row: len(row.old), default=None)


# --- reporting ---------------------------------------------------------------


def _counts(items: Iterable[Item]) -> str:
    """How many tests and other Rust items `items` hold."""
    items = [item for item in items if item.kind in RUST_KINDS]
    tests = sum(item.tests for item in items)
    return f"{tests} tests, {sum(1 for item in items if not item.tests)} helpers"


def _others(items: Iterable[Item]) -> str:
    kinds = Counter(item.kind for item in items)
    return f"{kinds['fixture']} fixtures and {kinds['target']} target stanzas"


def report(result: Result, moves: bool = False, out=None, err=None) -> int:
    """Print the result: counts and moves to `out`, findings to `err`. Its exit code."""
    out, err = out or sys.stdout, err or sys.stderr
    by_file: dict[str, list[Item]] = defaultdict(list)
    for item in result.new.values():
        if item.kind in RUST_KINDS:
            by_file[item.file].append(item)
    old_by_file: dict[str, list[Item]] = defaultdict(list)
    for item in result.old.values():
        if item.kind in RUST_KINDS:
            old_by_file[item.file].append(item)
    print(f"Test-context items per file, at {result.baseline} and at HEAD:", file=out)
    seen_new = set()
    for path in sorted(old_by_file):
        target = result.file_pairs.get(path)
        after = _counts(by_file.get(target, [])) if target else "gone"
        seen_new.add(target)
        name = path if target in (path, None) else f"{path} -> {target}"
        print(f"  {name}: {_counts(old_by_file[path])} -> {after}", file=out)
    for path in sorted(set(by_file) - seen_new):
        print(f"  {path} (new): {_counts(by_file[path])}", file=out)

    for found, entry in result.acknowledged:
        print(f"Acknowledged as DRIFT {entry.row}: {found.old.where} -> {found.new.where} "
              f"(sha256 {found.sha256})", file=out)
    for item, entry in result.retired:
        print(f"Retired with the manifest's `{entry.row}`: {item.where}", file=out)
    for found in result.diffs:
        print(f"::error::{found.old.where} differs at {found.new.file}:{found.new.line} after "
              f"normalising (sha256 {found.sha256}). Restore it, or acknowledge it with a "
              f"`[[drift]]` entry in {CONFIG} naming its DRIFT row:", file=err)
        for line in found.lines:
            print(f"  {line}", file=err)
    for item in result.lost:
        print(f"::error::{item.where} has no counterpart at HEAD. Restore it, pair its file "
              f"with `[files]`, or retire it with a `[[retired]]` entry citing the manifest "
              f"row whose reason covers its subject.", file=err)
    for problem in result.problems:
        print(f"::error::{problem}", file=err)
    for item in result.added:
        print(f"New at HEAD: {item.where}", file=out)
    if moves:
        for old, new in sorted(result.pairs, key=lambda pair: pair[0].key):
            if old.kind in RUST_KINDS and (old.file, old.line) != (new.file, new.line):
                print(f"{old.file}:{old.line} {old.name} -> {new.file}:{new.line}", file=out)

    print(f"{_counts(result.old.values())}, {_others(result.old.values())} at "
          f"{result.baseline}; "
          f"{len(result.pairs)} paired, {len(result.diffs) + len(result.acknowledged)} "
          f"differ ({len(result.acknowledged)} acknowledged), "
          f"{len(result.lost) + len(result.retired)} lost ({len(result.retired)} retired), "
          f"{len(result.added)} new at HEAD, {len(result.problems)} entries stale or invalid.",
          file=out)
    return result.status


# --- the command line --------------------------------------------------------


def _manifest(root: Path):
    try:
        import api_removals
    except ImportError as error:
        raise BodyDiffError(f"could not import scripts/api_removals.py: {error}") from error
    try:
        return api_removals.read_manifest(root)
    except api_removals.RemovalsError as error:
        raise BodyDiffError(str(error)) from error


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Compare every test-context item at a baseline with its counterpart "
                    "at HEAD, normalised through docs/rename-manifest.md.")
    parser.add_argument("--baseline", metavar="REV",
                        help="the git revision to compare with; the manifest's baseline by "
                             "default")
    parser.add_argument("--moves", action="store_true",
                        help="print old file:line name -> new file:line for every paired "
                             "item that moved, for docs/MUTANTS.md")
    return parser


def main(argv: list[str] | None = None, root: Path = REPO) -> int:
    args = _parser().parse_args(argv)
    try:
        manifest = _manifest(root)
        baseline = args.baseline or manifest.baseline
        if baseline != manifest.baseline:
            print(f"::warning::docs/rename-manifest.md maps paths from {manifest.baseline}, "
                  f"not {baseline}, so its rows may not apply", file=sys.stderr)
        config = read_config(root)
        valid = frozenset()
        if config.drift:
            try:
                valid = drift_rows((root / DIVERGENCES).read_text(encoding="utf-8"))
            except OSError as error:
                raise BodyDiffError(f"could not read {DIVERGENCES}: {error}") from error
        result = compare(root, baseline, tuple(manifest.rows), config, valid,
                         features=tuple(manifest.features))
        return report(result, args.moves)
    except BodyDiffError as error:
        print(f"::error::{error}", file=sys.stderr)
        return 2
    # An unreadable file or malformed text says nothing about the tests, so it must
    # not read as the diff failing.
    except (OSError, ValueError, KeyError) as error:
        print(f"::error::cannot decide: {type(error).__name__}: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
