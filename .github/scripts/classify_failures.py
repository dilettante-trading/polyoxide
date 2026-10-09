#!/usr/bin/env python3
"""Classify cargo nextest failures into auth-gated / environmental / transient / real.

A failure that went through `polyoxide-test-support` says which it is: the test
prints `polyoxide-class=<tag>` alone on a line just before it panics, and that
line decides. A log with no tag line is `real`. Nothing reads the panic text:
the regexes that once guessed a failure's kind from it are gone, and
`scripts/live_unwraps.py` freezes this module's regexes so none comes back. A
new failure mode is tagged where it fails instead, and the same script holds
every live test's unwraps and bare panics at zero, so each failure is tagged or
deliberately left `real`.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from enum import Enum
from pathlib import Path


class Verdict(str, Enum):
    PASS = "pass"
    AUTH_GATED = "auth-gated"
    ENVIRONMENTAL = "environmental"
    TRANSIENT = "transient"
    REAL = "real"


# The line `polyoxide-test-support` prints before a test panics. It must be
# alone on its line: a tag quoted inside other text, such as a panic message,
# is not one.
TAG_LINE = re.compile(r"^polyoxide-class=(\S+)\s*$", re.MULTILINE)
TAGS = {v.value: v for v in (Verdict.AUTH_GATED, Verdict.ENVIRONMENTAL, Verdict.TRANSIENT, Verdict.REAL)}
# The first line of the report Rust's default panic hook prints, which the
# test-support hook calls right after printing the tag.
PANIC_REPORT = re.compile(r"^thread '.*' (?:\(\d+\) )?panicked at ")


def _final_tag(failure_output: str) -> str | None:
    """The tag of the panic that failed the test: the tag line just before the
    last panic report, blank lines aside, or `None` when there is none there.

    A tag anywhere else belongs to an earlier panic, one the test caught or a
    spawned task raised, and says nothing about how the test itself failed.
    """
    lines = failure_output.splitlines()
    reports = [i for i, line in enumerate(lines) if PANIC_REPORT.match(line)]
    if not reports:
        return None
    above = reports[-1] - 1
    while above >= 0 and not lines[above].strip():
        above -= 1
    tag = TAG_LINE.match(lines[above]) if above >= 0 else None
    return tag[1] if tag else None


def classify(failure_output: str) -> Verdict:
    """Classify a single failure's combined stdout+stderr text.

    A tag line decides when it is the one just before the last panic report,
    which is where the test-support hook prints it; a tag this script does not
    know is REAL. Any tag that is REAL, wherever it is, makes the failure REAL:
    a fault the test caught, or a spawned task hit, is still a fault. A log
    with no tag there is REAL too, whatever its text says: a failure nothing
    tagged is one nobody decided is safe to skip or retry.
    """
    if any(TAGS.get(tag, Verdict.REAL) == Verdict.REAL for tag in TAG_LINE.findall(failure_output)):
        return Verdict.REAL
    tag = _final_tag(failure_output)
    return Verdict.REAL if tag is None else TAGS[tag]


@dataclass(frozen=True)
class TestOutcome:
    name: str
    verdict: Verdict
    output: str  # raw stdout+stderr; empty for PASS


# The attempt number nextest appends to a retried test's name.
ATTEMPT = re.compile(r"#\d+$")


def _joined(stdout: str, stderr: str) -> str:
    """`stdout` then `stderr`, with a newline between them when `stdout` lacks
    one, so a tag line opening `stderr` still starts a line."""
    if stdout and stderr and not stdout.endswith("\n"):
        return f"{stdout}\n{stderr}"
    return stdout + stderr


def parse_nextest_json(path: Path) -> list[TestOutcome]:
    """Parse a nextest libtest-json NDJSON file into TestOutcomes.

    Each line is a JSON object. We care about events with `type == "test"`
    and `event in ("ok", "failed")`. Other events (suite-level, started)
    are ignored.

    nextest appends `#<attempt>` to the name of a test it ran more than once,
    as the retry pass does (`crate::binary$live_x#3`). The suffix is dropped,
    so `merge` finds a retried test under the name the first pass gave it.
    """
    outcomes: list[TestOutcome] = []
    with path.open() as f:
        for raw_line in f:
            line = raw_line.strip()
            if not line:
                continue
            event = json.loads(line)
            if event.get("type") != "test":
                continue
            kind = event.get("event")
            name = ATTEMPT.sub("", event.get("name", ""))
            if kind == "ok":
                outcomes.append(TestOutcome(name=name, verdict=Verdict.PASS, output=""))
            elif kind == "failed":
                output = _joined(event.get("stdout", ""), event.get("stderr", ""))
                outcomes.append(TestOutcome(name=name, verdict=classify(output), output=output))
    return outcomes


def retry_filterset(names: list[str]) -> str:
    """Build a nextest filterset expression selecting exactly `names`.

    libtest-json reports tests as `crate::binary$test`; `test(=...)` matches
    the bare test name only, so each clause pins the binary too. A name
    without a `$` (defensive) falls back to a bare `test(=...)` clause.
    """
    clauses: list[str] = []
    for name in names:
        binary_id, sep, test = name.partition("$")
        if sep:
            clauses.append(f"(binary_id(={binary_id}) & test(={test}))")
        else:
            clauses.append(f"test(={name})")
    return " | ".join(clauses)


def _write_lines(path: Path, names: list[str]) -> None:
    """Write one name per line; trailing newline only if non-empty."""
    if names:
        path.write_text("\n".join(names) + "\n")
    else:
        path.write_text("")


def _render_report(real_failures: list[TestOutcome]) -> str:
    if not real_failures:
        return "All real failures resolved on retry, or no failures observed.\n"
    lines = ["## Real failures", ""]
    for outcome in real_failures:
        lines.append(f"### `{outcome.name}`")
        lines.append("")
        lines.append("```")
        # Truncate very long outputs to keep issue bodies manageable.
        truncated = outcome.output if len(outcome.output) <= 4000 else outcome.output[:4000] + "\n... [truncated]"
        lines.append(truncated.rstrip())
        lines.append("```")
        lines.append("")
    return "\n".join(lines)


def _emit_outputs(outcomes: list[TestOutcome], output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    retry = [o.name for o in outcomes if o.verdict == Verdict.TRANSIENT]
    real = [o for o in outcomes if o.verdict == Verdict.REAL]
    auth = [o.name for o in outcomes if o.verdict == Verdict.AUTH_GATED]
    environmental = [o.name for o in outcomes if o.verdict == Verdict.ENVIRONMENTAL]
    _write_lines(output_dir / "retry-tests.txt", retry)
    filterset = retry_filterset(retry)
    (output_dir / "retry-filter.txt").write_text(filterset + "\n" if filterset else "")
    _write_lines(output_dir / "real-failures.txt", [o.name for o in real])
    _write_lines(output_dir / "auth-gated.txt", auth)
    _write_lines(output_dir / "environmental.txt", environmental)
    (output_dir / "report.md").write_text(_render_report(real))


def _cmd_classify(args: argparse.Namespace) -> int:
    outcomes = parse_nextest_json(args.input)
    _emit_outputs(outcomes, args.output_dir)
    return 0


def _cmd_merge(args: argparse.Namespace) -> int:
    """Merge first-pass and retry. A test is REAL iff it was REAL on first pass
    OR was TRANSIENT on first pass and (still TRANSIENT or REAL) on retry. A
    TRANSIENT whose retry is ENVIRONMENTAL or AUTH_GATED takes the retry's
    verdict: a dropped connection followed by a quiet feed is not a defect."""
    first = parse_nextest_json(args.first_pass)
    retry = parse_nextest_json(args.retry)
    by_name_retry = {o.name: o for o in retry}

    merged: list[TestOutcome] = []
    for o in first:
        if o.verdict in (Verdict.PASS, Verdict.AUTH_GATED, Verdict.ENVIRONMENTAL, Verdict.REAL):
            merged.append(o)
        elif o.verdict == Verdict.TRANSIENT:
            r = by_name_retry.get(o.name)
            if r is None or r.verdict == Verdict.PASS:
                merged.append(TestOutcome(o.name, Verdict.PASS, ""))
            elif r.verdict in (Verdict.ENVIRONMENTAL, Verdict.AUTH_GATED):
                merged.append(TestOutcome(o.name, r.verdict, r.output))
            else:
                # Still TRANSIENT or escalated to REAL — treat as REAL in the report.
                merged.append(TestOutcome(o.name, Verdict.REAL, r.output or o.output))

    # The merge command never emits a retry list (the retry already happened).
    output_dir = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    real = [o for o in merged if o.verdict == Verdict.REAL]
    auth = [o.name for o in merged if o.verdict == Verdict.AUTH_GATED]
    environmental = [o.name for o in merged if o.verdict == Verdict.ENVIRONMENTAL]
    _write_lines(output_dir / "retry-tests.txt", [])
    _write_lines(output_dir / "real-failures.txt", [o.name for o in real])
    _write_lines(output_dir / "auth-gated.txt", auth)
    _write_lines(output_dir / "environmental.txt", environmental)
    (output_dir / "report.md").write_text(_render_report(real))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Classify nextest failures.")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_classify = sub.add_parser("classify", help="First-pass classification.")
    p_classify.add_argument("--input", type=Path, required=True)
    p_classify.add_argument("--output-dir", type=Path, required=True)
    p_classify.set_defaults(func=_cmd_classify)

    p_merge = sub.add_parser("merge", help="Merge first-pass and retry results.")
    p_merge.add_argument("--first-pass", type=Path, required=True)
    p_merge.add_argument("--retry", type=Path, required=True)
    p_merge.add_argument("--output-dir", type=Path, required=True)
    p_merge.set_defaults(func=_cmd_merge)

    ns = parser.parse_args(argv)
    return ns.func(ns)


if __name__ == "__main__":
    sys.exit(main())
