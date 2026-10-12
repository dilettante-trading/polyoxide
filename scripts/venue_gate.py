#!/usr/bin/env python3
"""The venue gate (CAP-7): the foundation crates name no venue.

`polyoxide-venue`, `polyoxide-core`, `polyoxide-ws` and `polyoxide-test-support` are
what every venue builds on, so none of them may carry a venue's code, rules or
defaults. This script reads every file under them, case-insensitively, for

  - each venue id, as a substring: `polymarket` in `polymarket_limits`;
  - each product id, as identifier segments: the text is split at `_`, `-`, `.`, `::`
    and every other character that is not a letter or a digit, and at case
    boundaries, so `clob` is found in `clob_default`, `ClobThrottle` and `clob::`, and
    `perps` in `PERPS_GENERAL`, but not `clob` in `clobber`. The generic words of
    ci/venue-gate-skip.txt (`data`, `events`) are not looked for;
  - each `<venue>.<product>:` prefix, for each product its venue declares;
  - each venue's `identifiers`, as substrings: Polymarket's `Poly-RateLimit`.

The terms come from `cargo metadata`: each member's `[package.metadata.polyoxide]`
`venue`, `products` and `identifiers`. A term belongs to the venues that declare it.
A hit passes only through a `gate_exceptions` entry, `{ path, term, reason }`, in the
metadata of a crate of a venue the term belongs to, whose `path` is the hit's file or
a directory holding it and whose `term` is the term found. An entry that matches no
hit is stale and fails, so no exception outlives what it excuses.
`scripts/gen_registry.py` checks each entry's shape.

Exit codes: 0 when every hit is excepted and every exception matches a hit; 1 when a
hit is not excepted, or an exception is stale; 2 when the gate cannot decide (`cargo
metadata` failed, or the skip list or a crate's metadata cannot be read).

Stdlib only.

Usage:
    python3 scripts/venue_gate.py
"""
from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish_order  # noqa: E402
from gen_registry import FOUNDATION  # noqa: E402

REPO = publish_order.REPO
SKIP = "ci/venue-gate-skip.txt"
# What a crate's directory holds that is not its source.
IGNORED_DIRS = frozenset({"target", ".git"})
WORD = re.compile(r"[A-Za-z0-9]+")
CASE = re.compile(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z]+|[A-Z]+|[0-9]+")
# The text a hit is reported in: the identifier, or hyphenated word, around it.
AROUND = re.compile(r"[\w-]+")


class GateError(Exception):
    """The gate cannot decide. The message says why."""


@dataclass(frozen=True)
class Term:
    text: str  # lowercase
    kind: str  # "venue", "product", "prefix" or "identifier"
    venues: frozenset[str]


@dataclass(frozen=True)
class Excuse:
    """A `gate_exceptions` entry, with the crate and venue that declare it."""

    crate: str
    venue: str
    path: str
    term: str
    reason: str

    def covers(self, hit: Hit) -> bool:
        return (self.term.lower() == hit.term.text and self.venue in hit.term.venues
                and (hit.path == self.path or hit.path.startswith(self.path.rstrip("/") + "/")))


@dataclass(frozen=True)
class Hit:
    path: str  # from the repository root
    line: int
    term: Term
    word: str  # the identifier or word it was found in, as written
    text: str  # its line, stripped

    def __str__(self) -> str:
        return (f"{self.path}:{self.line}: {self.term.kind} {self.term.text!r} in "
                f"{self.word!r}: {self.text[:160]}")


@dataclass(frozen=True)
class Gate:
    terms: tuple[Term, ...]
    excuses: tuple[Excuse, ...]


@dataclass
class Result:
    hits: list[Hit]
    unexcused: list[Hit]
    stale: list[Excuse]

    @property
    def status(self) -> int:
        return 1 if self.unexcused or self.stale else 0


# --- the terms ---------------------------------------------------------------


def read_skip(root: Path) -> frozenset[str]:
    """The generic words no product segment is matched against: one per line, `#`
    starting a comment."""
    try:
        text = (root / SKIP).read_text(encoding="utf-8")
    except OSError as err:
        raise GateError(f"could not read {SKIP}: {err}") from err
    words = set()
    for line in text.splitlines():
        word = line.split("#", 1)[0].strip()
        if word:
            if not WORD.fullmatch(word):
                raise GateError(f"{SKIP}: {word!r} is not one word")
            words.add(word.lower())
    return frozenset(words)


def _list(name: str, key: str, value) -> list:
    if value is None:
        return []
    if not isinstance(value, list):
        raise GateError(f"{name}: [package.metadata.polyoxide] {key} must be a list")
    return value


def read_gate(metadata: dict, skip: Iterable[str] = ()) -> Gate:
    """The terms and exceptions every member's registration declares."""
    owners: dict[tuple[str, str], set[str]] = {}
    excuses = []

    def own(text: str, kind: str, venue: str) -> None:
        owners.setdefault((text.lower(), kind), set()).add(venue)

    products: dict[str, set[str]] = {}
    for package in publish_order.members(metadata):
        name = package["name"]
        meta = (package.get("metadata") or {}).get("polyoxide") or {}
        venue = meta.get("venue")
        if venue is None:
            continue
        if not isinstance(venue, str) or not venue:
            raise GateError(f"{name}: [package.metadata.polyoxide] venue must be a string")
        own(venue, "venue", venue)
        for product in _list(name, "products", meta.get("products")):
            products.setdefault(venue, set()).add(product.lower())
        for identifier in _list(name, "identifiers", meta.get("identifiers")):
            own(identifier, "identifier", venue)
        for entry in _list(name, "gate_exceptions", meta.get("gate_exceptions")):
            if not isinstance(entry, dict) or not all(
                    isinstance(entry.get(k), str) for k in ("path", "term", "reason")):
                raise GateError(f"{name}: each gate_exceptions entry needs a path, a term "
                                f"and a reason, as strings")
            excuses.append(Excuse(name, venue, entry["path"], entry["term"], entry["reason"]))
    skipped = {word.lower() for word in skip}
    for venue, names in products.items():
        for product in names:
            own(f"{venue}.{product}:", "prefix", venue)
            if product not in skipped:
                own(product, "product", venue)
    terms = tuple(Term(text, kind, frozenset(venues))
                  for (text, kind), venues in sorted(owners.items()))
    return Gate(terms, tuple(excuses))


# --- the scan ----------------------------------------------------------------


def _segments(line: str) -> list[tuple[int, str]]:
    """Each identifier segment of `line`, lowercased, with where it starts."""
    out = []
    for word in WORD.finditer(line):
        for part in CASE.finditer(word[0]):
            out.append((word.start() + part.start(), part[0].lower()))
    return out


def _around(line: str, at: int, length: int) -> str:
    """The word the text at `at` is part of."""
    for match in AROUND.finditer(line):
        if match.start() <= at < match.end():
            end = max(match.end(), at + length)
            return line[match.start():end]
    return line[at:at + length]


def scan_text(path: str, text: str, gate: Gate) -> list[Hit]:
    """Every hit of `gate`'s terms in `text`, the file at `path`."""
    hits = []
    substrings = [t for t in gate.terms if t.kind != "product"]
    products = [(t, t.text.split("-")) for t in gate.terms if t.kind == "product"]
    for number, line in enumerate(text.splitlines(), 1):
        lowered = line.lower()
        for term in substrings:
            start = lowered.find(term.text)
            while start != -1:
                hits.append(Hit(path, number, term, _around(line, start, len(term.text)),
                                line.strip()))
                start = lowered.find(term.text, start + 1)
        if not products:
            continue
        segments = _segments(line)
        texts = [s for _, s in segments]
        for term, parts in products:
            for i in range(len(texts) - len(parts) + 1):
                if texts[i:i + len(parts)] == parts:
                    at = segments[i][0]
                    hits.append(Hit(path, number, term, _around(line, at, len(parts[0])),
                                    line.strip()))
    return hits


def files(root: Path, crates: Iterable[str] = FOUNDATION) -> list[str]:
    """Every file under each crate's directory, from `root`, but build output."""
    found = []
    for crate in crates:
        base = root / crate
        if not base.is_dir():
            raise GateError(f"{crate}/ is not a directory under {root}")
        for path in sorted(base.rglob("*")):
            relative = path.relative_to(root)
            if path.is_file() and not IGNORED_DIRS.intersection(relative.parts):
                found.append(relative.as_posix())
    return found


def scan(root: Path, gate: Gate, crates: Iterable[str] = FOUNDATION) -> list[Hit]:
    hits = []
    for path in files(root, crates):
        text = (root / path).read_bytes().decode("utf-8", "replace")
        hits += scan_text(path, text, gate)
    return hits


def check(hits: list[Hit], excuses: Iterable[Excuse]) -> Result:
    excuses = tuple(excuses)
    used = set()
    unexcused = []
    for hit in hits:
        covering = [e for e in excuses if e.covers(hit)]
        if covering:
            used.update(covering)
        else:
            unexcused.append(hit)
    return Result(hits, unexcused, [e for e in excuses if e not in used])


def run(root: Path, metadata: dict | None = None) -> Result:
    if metadata is None:
        try:
            metadata = publish_order.cargo_metadata(root / "Cargo.toml")
        except publish_order.PublishOrderError as err:
            raise GateError(str(err)) from err
    gate = read_gate(metadata, read_skip(root))
    return check(scan(root, gate), gate.excuses)


# --- the command line --------------------------------------------------------


def report(result: Result, out=None, err=None) -> int:
    out, err = out or sys.stdout, err or sys.stderr
    for hit in result.unexcused:
        print(f"::error::{hit}", file=err)
    for excuse in result.stale:
        print(f"::error::{excuse.crate}'s gate exception for {excuse.term!r} in "
              f"{excuse.path} matches nothing; delete it ({excuse.reason})", file=err)
    if result.unexcused:
        print(f"A foundation crate ({', '.join(FOUNDATION)}) names a venue. Say it without "
              f"the venue's words, move the code to the venue's crate, or, where the word "
              f"must stay (a protected test name), add a gate_exceptions entry with a "
              f"reason to that venue's crate.", file=err)
    print(f"{len(result.hits)} hits, {len(result.hits) - len(result.unexcused)} excepted, "
          f"{len(result.unexcused)} not; {len(result.stale)} exceptions stale.", file=out)
    return result.status


def main(argv: list[str] | None = None, root: Path = REPO) -> int:
    argparse.ArgumentParser(description=__doc__.split("\n", 1)[0]).parse_args(argv)
    try:
        return report(run(root))
    except GateError as error:
        print(f"::error::{error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
