#!/usr/bin/env python3
"""The runtime gate: production code reaches time and tasks through the injected runtime.

Every timer, monotonic clock read and spawn in `polyoxide-venue`, `-core`, `-ws`,
`-polymarket`, `-binance` and `-kalshi` goes through the `polyoxide_rt::Runtime` its client
or socket was built with, so a build for a browser, which has neither tokio's clock nor its
executor, injects its own. This script holds that line over the `src/**/*.rs` of those six
crates. Clippy's `disallowed-methods` cannot: it would fire in every test and example too,
which use tokio legitimately. It reads each file's code (comments and the contents of string
literals are not code) for

  - `tokio::spawn` and any `tokio::task::spawn*` (`spawn_blocking`, `spawn_local`);
  - any `tokio::time` path (`tokio::time::sleep`, `use tokio::time::{..}`);
  - `Instant::now` and `SystemTime::now`, however qualified, called or passed as a function
    value;
  - a `use` tree rooted at `tokio`, read leaf by leaf, whatever its grouping or `as` aliases:
    a leaf `spawn`, `task` (the module, whose `task::spawn` would follow), `task::spawn*`,
    `time` or `time::*` is a hit (`use tokio::{spawn, time::sleep};`, `use tokio::task;`).

It skips what is not production code:

  - an item under an attribute whose `cfg` holds only in tests: `test`, or `all(..)` with an
    operand that does (`#[cfg(test)]`, `#[cfg(all(test, feature = ".."))]`); `any(test, ..)`
    and every other predicate that can be true without `test` is production code and is read.
    The item ends at its first `{..}` group or `;`, at a `,` outside brackets when it is a
    variant, field, parameter or match arm, or just before the bracket that closes the
    list it sits in. For a `mod x;` declaration the skip also covers the file it declares
    (`x.rs` or `x/mod.rs` in the module's directory, which an enclosing inline `mod y { .. }`
    extends to `y/`) and every file under it;
  - a file whose first item is `#![cfg(test)]`, and every file under its module;
  - `polyoxide-ws/src/test_server/` and every file named `scripted.rs`.

`#[path = ".."]` is not followed. `polyoxide-rt` is not scanned: it implements the runtime.

A line ending `// runtime-gate: <reason>` is exempt, as `scripts/live_unwraps.py`'s opt-out
is. The wall clock is the case in tree: a timestamp sent to a venue is never injected.

Each hit is printed on stdout as `<path>:<line>: <banned path>`, the path from the repository
root. Exit codes: 0 when there is no hit, 1 when there is one.

Stdlib only.

Usage:
    python3 scripts/runtime_gate.py
"""
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_registry  # noqa: E402

REPO = gen_registry.REPO
CRATES = ("polyoxide-venue", "polyoxide-core", "polyoxide-ws", "polyoxide-polymarket",
          "polyoxide-binance", "polyoxide-kalshi")
TEST_SERVER = "polyoxide-ws/src/test_server"
FIXTURE = "scripted.rs"
# Each banned path, as it is printed, and the code that spells it.
BANNED = (
    ("tokio::spawn", re.compile(r"\btokio\s*::\s*spawn\w*")),
    ("tokio::task::spawn", re.compile(r"\btokio\s*::\s*task\s*::\s*spawn\w*")),
    ("tokio::time::", re.compile(r"\btokio\s*::\s*time\b")),
    ("Instant::now(", re.compile(r"\bInstant\s*::\s*now\b")),
    ("SystemTime::now(", re.compile(r"\bSystemTime\s*::\s*now\b")),
)
OPT_OUT = re.compile(r"^\s*runtime-gate:\s*\S")
CFG = re.compile(r"#\s*(!?)\s*\[\s*cfg\s*\(")
MOD_NAME = re.compile(r"\bmod\s+(\w+)\s*$")
INLINE_MOD = re.compile(r"\bmod\s+(\w+)\s*\{")
USE = re.compile(r"\buse\b")
# `extern crate tokio as tk;`, which names the crate under another name; `as _` binds nothing.
EXTERN_ALIAS = re.compile(r"\bextern\s+crate\s+(?:::\s*)?tokio\s+as\s+(?!_\b)\w+")
ALIAS = "tokio alias"
USE_TOKEN = re.compile(r"[A-Za-z_][A-Za-z0-9_]*|::|[{},*]")
# What starts an item, as against a variant, field, parameter or match arm: an item's header
# may hold commas outside brackets (`fn f<A, B>() where A: X, B: Y`).
ITEM_START = re.compile(
    r"(?:\s|#\[[^\]]*\])*(?:pub\s*(?:\([^)]*\))?\s*)?"
    r"(?:fn|impl|struct|enum|trait|mod|use|type|const|static|unsafe|async|extern|union"
    r"|macro_rules)\b")
INNER_ATTRIBUTES = re.compile(r"(?:\s*#!\[[^\]]*\])*\s*")
# The files whose module directory is their own directory.
ROOT_FILES = frozenset({"mod.rs", "lib.rs", "main.rs"})


@dataclass(frozen=True, order=True)
class Hit:
    path: str
    line: int
    banned: str


@dataclass(frozen=True)
class Source:
    """One file as the gate reads it."""

    code: str  # the code, test items blanked, offsets and lines as in the file
    comments: list[str]  # the `//` comment text of each line
    # `mod x;` declarations under a test `cfg`, each as the names of the inline modules it
    # sits in, outermost first, then `x`
    test_modules: tuple[tuple[str, ...], ...]
    test_file: bool  # `#![cfg(test)]` before the first item


def only_in_tests(predicate: str) -> bool:
    """Whether a `cfg` predicate holds only when `test` does: `test`, an `all(..)` with an
    operand that does, an `any(..)` whose every operand does. A string's contents are
    blanked in the code view, so `feature = ".."` is an operand that does not."""
    tokens = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|[(),=]", predicate)
    at = 0

    def operand() -> bool:
        nonlocal at
        name = tokens[at]
        at += 1
        if at < len(tokens) and tokens[at] == "=":
            at += 1
            return False
        if at < len(tokens) and tokens[at] == "(":
            at += 1
            args: list[bool] = []
            while tokens[at] != ")":
                args.append(operand())
                if tokens[at] == ",":
                    at += 1
            at += 1
            if name == "all":
                return any(args)
            return name == "any" and bool(args) and all(args)
        return name == "test"

    try:
        return operand()
    except IndexError:
        return False


def _close(code: str, start: int) -> int:
    """The index just past the bracket that closes the one at `start`."""
    depth = 0
    for i in range(start, len(code)):
        if code[i] in "([{":
            depth += 1
        elif code[i] in ")]}":
            depth -= 1
            if depth == 0:
                return i + 1
    return len(code)


def _item(code: str, start: int) -> tuple[int, str, str]:
    """Where the thing an attribute decorates, which starts at `start`, ends; its header; and
    the character that ends the header (`{`, `;`, or empty at a `,`, at the bracket that
    closes the list it sits in, and at the end of the file).

    An item ends at its first `{..}` group or `;`. A variant, field, parameter or match arm
    also ends at a `,` outside brackets, and every kind at an unmatched `)`, `]` or `}`: an
    early stop leaves a little of the element read, which is the safe direction."""
    in_list = not ITEM_START.match(code, start)
    i = start
    while i < len(code):
        if code[i] in "([":
            i = _close(code, i)
        elif code[i] == "{":
            return _close(code, i), code[start:i], "{"
        elif code[i] == ";":
            return i + 1, code[start:i], ";"
        elif code[i] in ")]}":
            return i, code[start:i], ""
        elif code[i] == "," and in_list:
            return i + 1, code[start:i], ""
        else:
            i += 1
    return len(code), code[start:], ""


def _inline_modules(code: str, at: int) -> tuple[str, ...]:
    """The names of the inline `mod y { .. }` blocks around offset `at`, outermost first."""
    around = []
    for match in INLINE_MOD.finditer(code):
        if match.end() <= at < _close(code, match.end() - 1):
            around.append(match[1])
    return tuple(around)


def _use_leaves(code: str, start: int) -> list[tuple[tuple[str, ...], int, bool]]:
    """Each leaf path of the `use` tree that follows `start`, with its offset in `code` and
    whether it renames the path (`as` something but `_`). A `self` leaf is its prefix; a glob is
    the path `.., "*"`."""
    end = code.find(";", start)
    tokens = [(m[0], start + m.start()) for m in USE_TOKEN.finditer(code[start:end])] \
        if end >= 0 else []
    leaves: list[tuple[tuple[str, ...], int, bool]] = []

    def tree(at: int, prefix: tuple[str, ...]) -> int:
        while at < len(tokens) and tokens[at][0] == "::":
            at += 1
        if at >= len(tokens):
            return at
        text, offset = tokens[at]
        if text == "*":
            leaves.append((prefix + ("*",), offset, False))
            return at + 1
        if text == "{":
            at += 1
            while at < len(tokens) and tokens[at][0] != "}":
                at = tree(at, prefix)
                if at < len(tokens) and tokens[at][0] == ",":
                    at += 1
            return at + 1
        if text in ",}":
            return at + 1
        path = prefix if text == "self" else prefix + (text,)
        at += 1
        if at < len(tokens) and tokens[at][0] == "::":
            return tree(at + 1, path)
        renamed = False
        if at < len(tokens) and tokens[at][0] == "as":
            renamed = at + 1 < len(tokens) and tokens[at + 1][0] != "_"
            at += 2
        leaves.append((path, offset, renamed))
        return at

    tree(0, ())
    return leaves


def use_hits(code: str) -> list[tuple[int, str]]:
    """The offset and banned path of each leaf of a `use` tree rooted at `tokio` that is one
    of the banned paths (the plain regexes read a path only when it is written whole), and each
    renaming of the `tokio` root itself (`use tokio as tk;`, `use tokio::{self as tk};`,
    `extern crate tokio as tk;`), after which `tk::spawn` is written whole nowhere."""
    hits = [(match.start(), ALIAS) for match in EXTERN_ALIAS.finditer(code)]
    for match in USE.finditer(code):
        for path, offset, renamed in _use_leaves(code, match.end()):
            if path[:1] != ("tokio",):
                continue
            rest = path[1:]
            if not rest and renamed:
                hits.append((offset, ALIAS))
            elif rest[:1] == ("*",) or (rest and rest[0].startswith("spawn")):
                hits.append((offset, "tokio::spawn"))
            elif rest[:1] == ("task",) and (len(rest) == 1 or rest[1] == "*"
                                            or rest[1].startswith("spawn")):
                hits.append((offset, "tokio::task::spawn"))
            elif rest[:1] == ("time",):
                hits.append((offset, "tokio::time::"))
    return hits


def read(text: str) -> Source:
    kinds = gen_registry.lex(text)
    code = gen_registry.view(text, kinds, gen_registry.CODE)
    comments = gen_registry.view(text, kinds, gen_registry.LINE_COMMENT).split("\n")
    spans: list[tuple[int, int]] = []
    modules: list[tuple[str, ...]] = []
    test_file = False
    for match in CFG.finditer(code):
        close = _close(code, match.end() - 1)
        if not only_in_tests(code[match.end():close - 1]):
            continue
        if match[1]:
            test_file = test_file or bool(INNER_ATTRIBUTES.fullmatch(code[:match.start()]))
            continue
        bracket = code.find("]", close)
        if bracket < 0:
            continue
        end, header, stop = _item(code, bracket + 1)
        spans.append((match.start(), end))
        declared = MOD_NAME.search(header)
        if stop == ";" and declared:
            modules.append(_inline_modules(code, match.start()) + (declared[1],))
    chars = list(code)
    for start, end in spans:
        for i in range(start, end):
            if chars[i] != "\n":
                chars[i] = " "
    return Source("".join(chars), comments, tuple(modules), test_file)


def source_files(root: Path = REPO) -> list[Path]:
    """The `src/**/*.rs` of each scanned crate under `root`."""
    return [file for crate in CRATES for file in sorted((root / crate / "src").rglob("*.rs"))]


def scan(root: Path = REPO, opt_out: bool = True) -> list[Hit]:
    """Every banned path in `root`'s production code, less the lines that opt out
    (all of them when `opt_out` is false)."""
    files = source_files(root)
    sources = {file: read(file.read_text(encoding="utf-8")) for file in files}
    skipped_files: set[Path] = set()
    skipped_dirs: set[Path] = set()
    for file, source in sources.items():
        module_dir = file.parent if file.name in ROOT_FILES else file.parent / file.stem
        if source.test_file:
            skipped_files.add(file)
            skipped_dirs.add(module_dir)
        for names in source.test_modules:
            where = module_dir.joinpath(*names[:-1])
            skipped_files.add(where / f"{names[-1]}.rs")
            skipped_dirs.add(where / names[-1])
    hits: set[Hit] = set()
    for file, source in sources.items():
        path = file.relative_to(root).as_posix()
        if (file in skipped_files or skipped_dirs & set(file.parents)
                or path.startswith(TEST_SERVER + "/") or file.name == FIXTURE):
            continue
        found = [(match.start(), banned) for banned, pattern in BANNED
                 for match in pattern.finditer(source.code)]
        for offset, banned in found + use_hits(source.code):
            line = source.code.count("\n", 0, offset)
            if opt_out and OPT_OUT.match(source.comments[line]):
                continue
            hits.add(Hit(path, line + 1, banned))
    return sorted(hits)


def main(argv: list[str] | None = None, root: Path = REPO, out=None, err=None) -> int:
    argparse.ArgumentParser(description=__doc__.splitlines()[0]).parse_args(argv)
    out, err = out or sys.stdout, err or sys.stderr
    hits = scan(root)
    for hit in hits:
        print(f"{hit.path}:{hit.line}: {hit.banned}", file=out)
    if hits:
        print(f"runtime gate: {len(hits)} hit(s). Route each through the injected runtime, "
              f"or end the line with `// runtime-gate: <reason>`.", file=err)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
