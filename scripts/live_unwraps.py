#!/usr/bin/env python3
"""Live tests may only lose unwraps, and the nightly classifier may not gain a regex (AD-14).

A live test that fails through `.or_fail(ctx)`, `fail(ctx, &err)`, a
credential loader, `environmental(reason)` or `transient(reason)` from
`polyoxide-test-support` prints the tag the nightly classifier reads. One that
fails through `.unwrap()` or `.expect(..)` prints none, and only the
classifier's regexes can sort it. This script holds both halves of that in
place with `scripts/live_unwraps.baseline.json`:

- `unwraps`: the number of unwrap calls in each live test file,
  `polyoxide-*/tests/live_*.rs` and the `mod` files each declares: every
  `.unwrap()` and `.expect(`, and every `Result::unwrap`, `Result::expect`,
  `Option::unwrap` and `Option::expect` named as a path. Each count must equal
  the baseline's. One that rose is an unwrap to replace with `.or_fail(ctx)`;
  one that fell is a baseline to lower, so it cannot rise again unnoticed. A
  file with a count must be in the baseline, and a file at zero must not be.
  The regex fallback is deleted when this table is empty.
- `opted_out`: the same count for the lines that opt out (below), held the same
  way, so an opt-out is never silent: adding one means raising this table by
  hand, where a reviewer sees it.
- `classifier`: every regex `.github/scripts/classify_failures.py` builds, read
  from its source rather than by running it. A literal pattern passed to a `re`
  function (`compile`, `search`, `match`, `fullmatch`, `findall`, `finditer`,
  `split`, `sub`, `subn`) is recorded with its flags, a pattern or flags that
  are not literals as their source text, and any other item in a list, tuple
  or set that holds a regex as its source text too. Each is filed under the
  module-level statement it is in: an assignment's target, or `name()` for a
  function. The set is frozen: a new failure mode is tagged where it fails,
  never matched by a new regex. Only a removed literal pattern may be lowered.

Text cannot tell a polyoxide `Result` from any other, so every call counts;
over-counting is the safe direction. Comments and string literals are not
code, so nothing in them counts. A line that must keep its unwraps, such as one
that parses a constant, opts out with a trailing comment giving the reason, and
its calls count under `opted_out` instead:

    let url: Url = "https://example.test".parse().unwrap(); // live-unwraps: a constant

Exit codes: 0 when the tree matches the baseline, or the baseline was lowered;
1 when they differ, or `--lower` was refused; 2 for bad arguments (argparse's
own).

Stdlib only.

Usage:
    python3 scripts/live_unwraps.py            # check
    python3 scripts/live_unwraps.py --lower    # lower the baseline to the tree
"""
from __future__ import annotations

import argparse
import ast
import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_registry  # noqa: E402

REPO = gen_registry.REPO
BASELINE = REPO / "scripts" / "live_unwraps.baseline.json"
CLASSIFIER = REPO / ".github" / "scripts" / "classify_failures.py"

UNWRAP = re.compile(r"\.\s*(?:unwrap\s*\(\s*\)|expect\s*\()|\b(?:Result|Option)::(?:unwrap|expect)\b")
OPT_OUT = re.compile(r"^\s*live-unwraps:\s*\S")
# The `re` flags a pattern's spelling records, as inline-flag letters.
FLAGS = ((re.IGNORECASE, "i"), (re.MULTILINE, "m"), (re.DOTALL, "s"), (re.VERBOSE, "x"))
# Each `re` function that takes a pattern, with the position of its `flags`.
REGEX_FUNCTIONS = {"compile": 1, "search": 2, "match": 2, "fullmatch": 2, "findall": 2,
                   "finditer": 2, "split": 3, "sub": 4, "subn": 4}


# --- counting ----------------------------------------------------------------


def split_lines(source: str) -> list[tuple[str, str]]:
    """Each line of Rust `source` as (code, trailing `//` comment).

    The code keeps every token but loses comments and the contents of string and
    character literals, so nothing quoted or commented can look like a call.
    """
    kinds = gen_registry.lex(source)
    code = gen_registry.view(source, kinds, gen_registry.CODE).split("\n")
    comments = gen_registry.view(source, kinds, gen_registry.LINE_COMMENT).split("\n")
    return list(zip(code, comments))


def count(source: str) -> tuple[int, int]:
    """The unwrap calls in `source`'s code: (counted, on lines that opt out)."""
    counted = opted_out = 0
    for code, comment in split_lines(source):
        calls = len(UNWRAP.findall(code))
        if OPT_OUT.match(comment):
            opted_out += calls
        else:
            counted += calls
    return counted, opted_out


def live_files(root: Path) -> list[Path]:
    """Every live test target under `root` and the `mod` files each declares."""
    files: dict[Path, None] = {}
    for target in sorted(root.glob("polyoxide*/tests/live_*.rs")):
        for file in gen_registry.module_files(target):
            files[file] = None
    return list(files)


def counts(root: Path) -> tuple[dict[str, int], dict[str, int]]:
    """The counted and the opted-out calls of each live test file under `root`
    that has any, by path from `root`."""
    found = {f.relative_to(root).as_posix(): count(f.read_text(encoding="utf-8"))
             for f in live_files(root)}
    return ({path: n for path, (n, _) in sorted(found.items()) if n},
            {path: n for path, (_, n) in sorted(found.items()) if n})


# --- the classifier's regexes ----------------------------------------------------


def spell(pattern: str, flags: int = 0) -> str:
    """`pattern` with its flags written inline, as the baseline records it."""
    letters = "".join(letter for flag, letter in FLAGS if flags & flag)
    return f"(?{letters}){pattern}" if letters else pattern


def classifier_patterns(path: Path) -> dict[str, list[str]]:
    """Every regex the Python module at `path` builds, by the module-level
    statement it is in (see the module docs)."""
    tree = ast.parse(path.read_text(encoding="utf-8"))
    modules, functions = _re_names(tree)

    def function(node: ast.AST) -> str | None:
        """The `re` function `node` calls, if it calls one."""
        if not isinstance(node, ast.Call):
            return None
        if (isinstance(node.func, ast.Attribute) and isinstance(node.func.value, ast.Name)
                and node.func.value.id in modules and node.func.attr in REGEX_FUNCTIONS):
            return node.func.attr
        if isinstance(node.func, ast.Name):
            return functions.get(node.func.id)
        return None

    tables: dict[str, list[str]] = {}
    for statement in tree.body:
        entries = []
        for node in ast.walk(statement):
            if name := function(node):
                entries.append(_regex(node, name, modules))
            elif isinstance(node, (ast.List, ast.Tuple, ast.Set)) and any(map(function, node.elts)):
                entries += [f"<not a regex: {ast.unparse(e)}>" for e in node.elts if not function(e)]
        if entries:
            tables.setdefault(_owner(statement), []).extend(entries)
    return tables


def _re_names(tree: ast.Module) -> tuple[set[str], dict[str, str]]:
    """The names `re` is imported under, and each `re` function imported by name."""
    modules, functions = set(), {}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            modules |= {alias.asname or alias.name for alias in node.names if alias.name == "re"}
        elif isinstance(node, ast.ImportFrom) and node.module == "re":
            functions |= {alias.asname or alias.name: alias.name for alias in node.names
                          if alias.name in REGEX_FUNCTIONS}
    return modules, functions


def _owner(statement: ast.stmt) -> str:
    if isinstance(statement, ast.Assign):
        return ast.unparse(statement.targets[0])
    if isinstance(statement, ast.AnnAssign):
        return ast.unparse(statement.target)
    if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return f"{statement.name}()"
    if isinstance(statement, ast.ClassDef):
        return f"class {statement.name}"
    return "<module>"


def _argument(call: ast.Call, position: int, keyword: str) -> ast.expr | None:
    if len(call.args) > position:
        return call.args[position]
    return next((k.value for k in call.keywords if k.arg == keyword), None)


def _regex(call: ast.Call, name: str, modules: set[str]) -> str:
    """The spelling of the regex `call` builds, or its source when that is not
    a literal pattern with literal flags."""
    pattern = _argument(call, 0, "pattern")
    if not (isinstance(pattern, ast.Constant) and isinstance(pattern.value, str)):
        return f"<not a literal: {ast.unparse(call)}>"
    flags_node = _argument(call, REGEX_FUNCTIONS[name], "flags")
    flags = 0 if flags_node is None else _flags(flags_node, modules)
    if flags is None:
        return f"<not a literal: {ast.unparse(call)}>"
    return spell(pattern.value, flags)


def _flags(node: ast.expr, modules: set[str]) -> int | None:
    """The value of a flags expression such as `re.I | re.M`, or `None`."""
    if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id in modules:
        flag = getattr(re, node.attr, None)
        return int(flag) if isinstance(flag, re.RegexFlag) else None
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.BitOr):
        left, right = _flags(node.left, modules), _flags(node.right, modules)
        return None if left is None or right is None else left | right
    if isinstance(node, ast.Constant) and isinstance(node.value, int):
        return node.value
    return None


# --- the check ---------------------------------------------------------------


# A problem's kind, and the kinds `--lower` may resolve, since each only shrinks
# the baseline.
NEW_FILE, ROSE, FELL, ADDED, REMOVED = "new file", "rose", "fell", "added", "removed"
LOWERABLE = {FELL, REMOVED}


def unwrap_problems(actual: dict[str, int], baseline: dict[str, int],
                    opted_out: bool = False) -> list[tuple[str, str]]:
    """(kind, message) for each file whose count differs from its baseline,
    in the `unwraps` table or, with `opted_out`, the `opted_out` one."""
    table = "opted_out" if opted_out else "unwraps"
    found = []
    for path in sorted(set(actual) | set(baseline)):
        now, allowed = actual.get(path, 0), baseline.get(path)
        if opted_out and (allowed is None or now > allowed):
            found.append((NEW_FILE if allowed is None else ROSE,
                f"{path} opts {now} unwrap(s) out with `// live-unwraps:`, more than its "
                f"`opted_out` baseline of {allowed or 0}. An opt-out is an exception a "
                f"reviewer must see: use `.or_fail(ctx)`, or raise the entry in "
                f"{BASELINE.name} by hand."))
        elif allowed is None:
            found.append((NEW_FILE,
                f"{path} has {now} unwrap(s) and no baseline entry. Fail live tests with "
                f"`.or_fail(ctx)` from polyoxide-test-support, so the nightly classifier "
                f"reads a tag instead of guessing from the panic text."))
        elif now > allowed:
            found.append((ROSE,
                f"{path} has {now} unwraps, {now - allowed} more than its baseline of "
                f"{allowed}. Use `.or_fail(ctx)` from polyoxide-test-support for each new "
                f"one."))
        elif now < allowed:
            fix = f"lower its entry to {now}" if now else "remove its entry"
            found.append((FELL,
                f"{path} has {now} {'opted-out ' if opted_out else ''}unwraps, below its "
                f"`{table}` baseline of {allowed}: {fix} in {BASELINE.name}, so they "
                f"cannot come back unnoticed (`python3 scripts/live_unwraps.py --lower` "
                f"does it)."))
    return found


def pattern_problems(actual: dict[str, list[str]],
                     frozen: dict[str, list[str]]) -> list[tuple[str, str]]:
    """(kind, message) for each change to the classifier's regexes. Only a
    removed literal pattern is `REMOVED`; every other change is `ADDED`."""
    found = []
    for name in sorted(set(actual) | set(frozen)):
        now, then = Counter(actual.get(name, [])), Counter(frozen.get(name, []))
        for entry in sorted((now - then).elements()):
            found.append((ADDED,
                f"{CLASSIFIER.name}: {name} gained `{entry}`. No regex may be added to "
                f"the classifier (AD-14): tag the failure where it happens, with "
                f"`.or_fail(ctx)`, `fail(ctx, &err)`, a credential loader, "
                f"`environmental(reason)` or `transient(reason)` from polyoxide-test-support."))
        for entry in sorted((then - now).elements()):
            if entry.startswith("<"):
                found.append((ADDED,
                    f"{CLASSIFIER.name}: {name} no longer holds `{entry}`, which is not a "
                    f"literal regex, so the change cannot be lowered: edit "
                    f"{BASELINE.name}'s `classifier` by hand."))
            else:
                found.append((REMOVED,
                    f"{CLASSIFIER.name}: {name} no longer holds the regex `{entry}`: drop "
                    f"it from {BASELINE.name}'s `classifier` too "
                    f"(`python3 scripts/live_unwraps.py --lower` does it)."))
    return found


def read_baseline(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def write_baseline(path: Path, unwraps: dict[str, int], opted_out: dict[str, int],
                   classifier: dict[str, list[str]]) -> None:
    text = json.dumps({"unwraps": unwraps, "opted_out": opted_out, "classifier": classifier},
                      indent=2, ensure_ascii=False)
    path.write_text(text + "\n", encoding="utf-8")


def problems(root: Path = REPO, baseline: Path = BASELINE,
             classifier: Path = CLASSIFIER) -> list[tuple[str, str]]:
    """(kind, message) for every way the tree under `root` differs from `baseline`."""
    frozen = read_baseline(baseline)
    unwraps, opted_out = counts(root)
    return (unwrap_problems(unwraps, frozen["unwraps"])
            + unwrap_problems(opted_out, frozen["opted_out"], opted_out=True)
            + pattern_problems(classifier_patterns(classifier), frozen["classifier"]))


def check(root: Path = REPO, baseline: Path = BASELINE, classifier: Path = CLASSIFIER) -> list[str]:
    """Every way the tree under `root` differs from `baseline`, as messages."""
    return [message for _, message in problems(root, baseline, classifier)]


def lower(root: Path = REPO, baseline: Path = BASELINE, classifier: Path = CLASSIFIER) -> list[str]:
    """Lowers `baseline` to the tree, or returns why it may not: a count that
    rose, a file without an entry, or any change to the classifier's regexes
    but a removed literal pattern."""
    refused = [message for kind, message in problems(root, baseline, classifier)
               if kind not in LOWERABLE]
    if not refused:
        write_baseline(baseline, *counts(root), classifier_patterns(classifier))
    return refused


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--lower", action="store_true",
                        help="rewrite the baseline to the tree's counts and regexes; refused "
                             "if any count rose, any file is new, or any regex was added")
    args = parser.parse_args(argv)
    if args.lower:
        refused = lower()
        for problem in refused:
            print(f"::error::{problem}")
        if not refused:
            print(f"lowered {BASELINE.relative_to(REPO)} to the tree")
        return 1 if refused else 0
    found = check()
    for problem in found:
        print(f"::error::{problem}")
    if found:
        return 1
    frozen = read_baseline(BASELINE)
    total = sum(frozen["unwraps"].values())
    opted = sum(frozen["opted_out"].values())
    regexes = sum(len(v) for v in frozen["classifier"].values())
    print(f"live unwraps: {total} in {len(frozen['unwraps'])} files and {opted} opted out, "
          f"as the baseline allows; classifier: {regexes} regexes, none added")
    return 0


if __name__ == "__main__":
    sys.exit(main())
