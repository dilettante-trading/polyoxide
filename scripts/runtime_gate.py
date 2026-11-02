#!/usr/bin/env python3
"""The runtime gate: production code reaches time and tasks through the injected runtime.

Every timer, monotonic clock read and spawn in `polyoxide-venue`, `-core`, `-ws`,
`-polymarket`, `-binance` and `-kalshi` goes through the `polyoxide_rt::Runtime` its client
or socket was built with, so a build for a browser, which has neither tokio's clock nor its
executor, injects its own. This script holds that line over the `src/**/*.rs` of those six
crates. Clippy's `disallowed-methods` cannot: it would fire in every test and example too,
which use tokio legitimately. It reads each file's code (comments and the contents of string
literals are not code) for

  - `tokio::spawn` and `tokio::task::spawn`;
  - any `tokio::time` path (`tokio::time::sleep`, `use tokio::time::{..}`);
  - `Instant::now(` and `SystemTime::now(`, however qualified.

It skips what is not production code:

  - an item under an attribute whose `cfg` names `test` outside a `not(..)` (`#[cfg(test)]`,
    `#[cfg(any(test, feature = ".."))]`), and, for a `mod x;` declaration, the file it
    declares (`x.rs` or `x/mod.rs` beside the declaring file) and every file under it;
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
    ("tokio::spawn", re.compile(r"\btokio\s*::\s*spawn\b")),
    ("tokio::task::spawn", re.compile(r"\btokio\s*::\s*task\s*::\s*spawn\b")),
    ("tokio::time::", re.compile(r"\btokio\s*::\s*time\b")),
    ("Instant::now(", re.compile(r"\bInstant\s*::\s*now\s*\(")),
    ("SystemTime::now(", re.compile(r"\bSystemTime\s*::\s*now\s*\(")),
)
OPT_OUT = re.compile(r"^\s*runtime-gate:\s*\S")
CFG = re.compile(r"#\s*(!?)\s*\[\s*cfg\s*\(")
MOD_NAME = re.compile(r"\bmod\s+(\w+)\s*$")
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
    test_modules: tuple[str, ...]  # `mod x;` declarations under a test `cfg`
    test_file: bool  # `#![cfg(test)]` before the first item


def names_test(predicate: str) -> bool:
    """Whether a `cfg` predicate names `test` outside every `not(..)`."""
    nots: list[bool] = []
    previous = ""
    for token in re.findall(r"[A-Za-z_][A-Za-z0-9_]*|[()]", predicate):
        if token == "(":
            nots.append(previous == "not")
        elif token == ")":
            if nots:
                nots.pop()
        elif token == "test" and not any(nots):
            return True
        previous = token
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
    """Where the item that starts at `start` ends, its header, and the character that
    ends the header (`{`, `;` or empty at the end of the file)."""
    i = start
    while i < len(code):
        if code[i] in "([":
            i = _close(code, i)
        elif code[i] == "{":
            return _close(code, i), code[start:i], "{"
        elif code[i] == ";":
            return i + 1, code[start:i], ";"
        else:
            i += 1
    return len(code), code[start:], ""


def read(text: str) -> Source:
    kinds = gen_registry.lex(text)
    code = gen_registry.view(text, kinds, gen_registry.CODE)
    comments = gen_registry.view(text, kinds, gen_registry.LINE_COMMENT).split("\n")
    spans: list[tuple[int, int]] = []
    modules: list[str] = []
    test_file = False
    for match in CFG.finditer(code):
        close = _close(code, match.end() - 1)
        if not names_test(code[match.end():close - 1]):
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
            modules.append(declared[1])
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
        for name in source.test_modules:
            skipped_files.add(module_dir / f"{name}.rs")
            skipped_dirs.add(module_dir / name)
    hits: set[Hit] = set()
    for file, source in sources.items():
        path = file.relative_to(root).as_posix()
        if (file in skipped_files or skipped_dirs & set(file.parents)
                or path.startswith(TEST_SERVER + "/") or file.name == FIXTURE):
            continue
        for banned, pattern in BANNED:
            for match in pattern.finditer(source.code):
                line = source.code.count("\n", 0, match.start())
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
