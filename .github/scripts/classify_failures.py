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
    ENVIRONMENTAL = "environmental"
    TRANSIENT = "transient"
    REAL = "real"


# Two shapes in the tree: live_api's `POLYMARKET_* env vars required for
# authenticated tests` and live_ws's `POLYMARKET_PRIVATE_KEY required; ...`.
AUTH_GATED_RE = re.compile(r"POLYMARKET_(?:\*|[A-Z_]+) (?:env vars )?required", re.IGNORECASE)

# Tests that depend on the state of the world (e.g. the sports channel with no
# match live anywhere) announce it in their panic message. Retrying within the
# same run won't change the world, and filing an issue would be a false
# positive — so these are logged and skipped, like auth-gated tests.
#
# Two shapes so far: the sports channel's `legitimately time out`, and the
# order-placing tests refusing to post because no open market's book satisfies
# their price precondition (`no qualifying market` from the selection helper,
# `no suitable market` from a test's own guard). The phrase is required in
# full — a bare `market` would swallow most genuine CLOB failures.
ENVIRONMENTAL_RE = re.compile(
    r"legitimately time out|no (?:qualifying|suitable) market",
    re.IGNORECASE,
)

# The transient set mirrors `ApiError::is_retriable()` in polyoxide-core, which
# is the SDK's canonical answer to "could re-sending change this?". This script
# cannot call it, so each arm is matched by prose instead — and every arm needs
# *two* spellings, because a panic can render its error either way:
#
#   .expect("...")          formats with `{:?}`  → Debug
#   panic!("...: {e}")      formats with `{}`    → Display
#
# The two disagree completely for `Network`. reqwest's Display for any
# request-phase failure is the bare phrase "error sending request" — it never
# names the cause — while its Debug prints the source chain, where a timeout
# appears as the unit struct `TimedOut` and a connect failure as a hyper-util
# error tagged `Connect`. The original table matched neither, so the genuine
# connect timeout in issue #32 was reported as a real failure.
#
# Misclassifying transient-as-real is the expensive direction: it files an issue
# immediately. The reverse is self-correcting, because `merge` promotes a
# transient that is still failing on retry back to real.
TRANSIENT_RES: list[re.Pattern[str]] = [
    # ApiError::RateLimit — Display "Rate limit exceeded: {0}" spaces the words,
    # Debug `RateLimit("…")` does not, so neither spelling matches the other.
    re.compile(r"\bToo Many Requests\b", re.IGNORECASE),
    re.compile(r"\brate limit\b", re.IGNORECASE),
    re.compile(r"\bRateLimit\("),
    # ApiError::Api { status } for 425 Too Early and 5xx, plus 429 before it is
    # narrowed to RateLimit. Display is "API error: {status} - {message}", Debug
    # is `Api { status: 503, .. }`, and reqwest's own status prose says "HTTP 503".
    re.compile(r"\b(?:HTTP|status:|API error:)\s*(?:425|429|5\d{2})\b", re.IGNORECASE),
    # ApiError::Timeout (HTTP 408) — Display "Request timeout"; Debug is the bare
    # unit variant, matched only through the wrapper the crate errors add, since
    # `Timeout` on its own is too common a word in ordinary panic prose.
    re.compile(r"\brequest timeout\b", re.IGNORECASE),
    re.compile(r"\bApi\(Timeout\)"),
    # ApiError::Network(e) where e.is_timeout(). reqwest's marker struct and
    # io::ErrorKind share the name, so one token covers `source: TimedOut`,
    # `kind: TimedOut` and `Kind(TimedOut)`. Case-sensitive: the Display strings
    # below are the separate, spaced spelling.
    re.compile(r"\bTimedOut\b"),
    re.compile(r"\b(?:request|operation) timed out\b", re.IGNORECASE),
    # ApiError::Network(e) where e.is_connect(). reqwest decides this by walking
    # its source chain for a hyper-util error whose kind is `Connect`; that type
    # Debug-prints as a tuple named for itself with the kind first, so this
    # matches exactly what `is_connect()` does.
    re.compile(r"hyper_util::client::legacy::Error\(Connect\b"),
    re.compile(r"\btcp connect error\b", re.IGNORECASE),
    # Under Display both Network arms collapse to this one phrase — it is
    # reqwest's entire vocabulary for a request-phase failure.
    re.compile(r"\berror sending request\b", re.IGNORECASE),
    # Socket-level causes, reached through either Network arm. An OS-backed
    # io::Error carries its message in both renderings.
    re.compile(r"\bConnection refused\b", re.IGNORECASE),
    re.compile(r"\bConnection reset by peer\b", re.IGNORECASE),
    re.compile(r"\bbroken pipe\b", re.IGNORECASE),
    # DNS, which fails before either arm can be decided.
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
    if ENVIRONMENTAL_RE.search(failure_output):
        return Verdict.ENVIRONMENTAL
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
    OR was TRANSIENT on first pass and (still TRANSIENT or REAL) on retry."""
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
