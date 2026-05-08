#!/usr/bin/env python3
"""Classify cargo nextest failures into auth-gated / transient / real."""

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
    TRANSIENT = "transient"
    REAL = "real"


AUTH_GATED_RE = re.compile(r"POLYMARKET_\* env vars required", re.IGNORECASE)

TRANSIENT_RES: list[re.Pattern[str]] = [
    re.compile(r"\bHTTP 429\b", re.IGNORECASE),
    re.compile(r"\bstatus:\s*429\b", re.IGNORECASE),
    re.compile(r"\bToo Many Requests\b", re.IGNORECASE),
    re.compile(r"\brate limit\b", re.IGNORECASE),
    re.compile(r"\bHTTP 5\d{2}\b", re.IGNORECASE),
    re.compile(r"\bstatus:\s*5\d{2}\b", re.IGNORECASE),
    re.compile(r"\bConnection refused\b", re.IGNORECASE),
    re.compile(r"\bConnection reset by peer\b", re.IGNORECASE),
    re.compile(r"\bbroken pipe\b", re.IGNORECASE),
    re.compile(r"\brequest timed out\b", re.IGNORECASE),
    re.compile(r"\boperation timed out\b", re.IGNORECASE),
    re.compile(r"\bDNS lookup failed\b", re.IGNORECASE),
    re.compile(r"\bfailed to lookup address\b", re.IGNORECASE),
    re.compile(r"\bname resolution failed\b", re.IGNORECASE),
]


def classify(failure_output: str) -> Verdict:
    """Classify a single failure's combined stdout+stderr text.

    Auth-gated takes precedence over transient (an auth panic message could
    plausibly contain a substring matching a transient pattern; we want the
    auth verdict in that case).
    """
    if AUTH_GATED_RE.search(failure_output):
        return Verdict.AUTH_GATED
    for pat in TRANSIENT_RES:
        if pat.search(failure_output):
            return Verdict.TRANSIENT
    return Verdict.REAL


@dataclass(frozen=True)
class TestOutcome:
    name: str
    verdict: Verdict
    output: str  # raw stdout+stderr; empty for PASS


def parse_nextest_json(path: Path) -> list[TestOutcome]:
    """Parse a nextest libtest-json NDJSON file into TestOutcomes.

    Each line is a JSON object. We care about events with `type == "test"`
    and `event in ("ok", "failed")`. Other events (suite-level, started)
    are ignored.
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
            name = event.get("name", "")
            if kind == "ok":
                outcomes.append(TestOutcome(name=name, verdict=Verdict.PASS, output=""))
            elif kind == "failed":
                output = event.get("stdout", "") + event.get("stderr", "")
                outcomes.append(TestOutcome(name=name, verdict=classify(output), output=output))
    return outcomes


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
    _write_lines(output_dir / "retry-tests.txt", retry)
    _write_lines(output_dir / "real-failures.txt", [o.name for o in real])
    _write_lines(output_dir / "auth-gated.txt", auth)
    (output_dir / "report.md").write_text(_render_report(real))


def _cmd_classify(args: argparse.Namespace) -> int:
    outcomes = parse_nextest_json(args.input)
    _emit_outputs(outcomes, args.output_dir)
    return 0


def _cmd_merge(args: argparse.Namespace) -> int:
    """Merge first-pass and retry. A test is REAL iff it was REAL on first pass
    OR was TRANSIENT on first pass and (still TRANSIENT or REAL) on retry."""
    first = parse_nextest_json(args.first_pass)
    retry = parse_nextest_json(args.retry)
    by_name_retry = {o.name: o for o in retry}

    merged: list[TestOutcome] = []
    for o in first:
        if o.verdict in (Verdict.PASS, Verdict.AUTH_GATED, Verdict.REAL):
            merged.append(o)
        elif o.verdict == Verdict.TRANSIENT:
            r = by_name_retry.get(o.name)
            if r is None or r.verdict == Verdict.PASS:
                merged.append(TestOutcome(o.name, Verdict.PASS, ""))
            else:
                # Still TRANSIENT or escalated to REAL — treat as REAL in the report.
                merged.append(TestOutcome(o.name, Verdict.REAL, r.output or o.output))

    # The merge command never emits a retry list (the retry already happened).
    output_dir = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    real = [o for o in merged if o.verdict == Verdict.REAL]
    auth = [o.name for o in merged if o.verdict == Verdict.AUTH_GATED]
    _write_lines(output_dir / "retry-tests.txt", [])
    _write_lines(output_dir / "real-failures.txt", [o.name for o in real])
    _write_lines(output_dir / "auth-gated.txt", auth)
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
