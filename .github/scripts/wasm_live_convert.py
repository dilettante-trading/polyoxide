#!/usr/bin/env python3
"""Turn wasm-bindgen-test-runner's report into the libtest-json the classifier reads.

`scripts/wasm_live.sh` runs the nightly's browser checks and calls this on the runner's log:

    python3 -I .github/scripts/wasm_live_convert.py <log> <json>

The classifier (`classify_failures.py`) was written for nextest, so each test becomes one
`{"type": "test", "name": "<binary>$<test>", "event": "ok"|"failed"}` line. A failed test carries
the runner's output block for it as `stdout`: dedented, with the panic report the hook leaves
(`panicked at ...`) given the `thread 'main' panicked at ...` shape the classifier looks for, so the
`polyoxide-class=<tag>` line stays just above it.

The runner's own summary (`test result: ... N passed; M failed; K ignored`) is read too, and the
counts must equal the results parsed. A runner that stops early (a timeout, a trap that leaves the
executor unusable for the tests still running) reports fewer tests than it ran, and a missing test
reads to the classifier as nothing to report: a check that never ran would pass. Exits 1 with a
message on stderr when the log holds no results, no summary, or counts that disagree.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

BINARY = "polyoxide-wasm-smoke::browser_live"

# The runner spells a failure `FAIL`; libtest's `FAILED` is read too.
RESULT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|FAIL|ignored)\s*$")
HEADING = re.compile(r"^---- (\S+) output ----\s*$")
PANIC = re.compile(r"^panicked at ")
SUMMARY = re.compile(r"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored\b")


class ConvertError(Exception):
    """The log cannot be trusted to hold every test that ran."""


def parse(text: str) -> tuple[dict[str, str], dict[str, list[str]], int | None]:
    """The results by test name, each failed test's output block, and the summary's total
    (`passed + failed + ignored`, summed over every summary line; `None` when there is none)."""
    lines = text.splitlines()

    results: dict[str, str] = {}
    total: int | None = None
    for line in lines:
        text_line = line.strip()
        found = RESULT.match(text_line)
        if found:
            results[found[1]] = found[2]
        summary = SUMMARY.match(text_line)
        if summary:
            total = (total or 0) + sum(int(n) for n in summary.groups())

    blocks: dict[str, list[str]] = {}
    current = None
    for line in lines:
        heading = HEADING.match(line.strip())
        if heading:
            current = heading[1]
            blocks[current] = []
            continue
        if current is None:
            continue
        text_line = line.strip()
        if text_line.startswith(("failures:", "test result:")):
            current = None
            continue
        if PANIC.match(text_line):
            text_line = "thread 'main' " + text_line
        blocks[current].append(text_line)
    return results, blocks, total


def events(text: str) -> list[dict]:
    """The libtest-json events for the runner's log; raises `ConvertError` when it is not whole."""
    results, blocks, total = parse(text)
    if not results:
        raise ConvertError("the runner printed no test results; the build or the runner failed")
    if total is None:
        raise ConvertError("the runner printed no `test result:` summary; it stopped before the end")
    if total != len(results):
        raise ConvertError(
            f"the runner's summary counts {total} tests but its log holds {len(results)} results; "
            "a test that never reported would read as a pass"
        )
    out = []
    for name, outcome in results.items():
        if outcome == "ignored":
            continue
        event = {
            "type": "test",
            "name": f"{BINARY}${name.rsplit('::', 1)[-1]}",
            "event": "ok" if outcome == "ok" else "failed",
        }
        if outcome != "ok":
            event["stdout"] = "\n".join(blocks.get(name, [])) + "\n"
        out.append(event)
    return out


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: wasm_live_convert.py <runner log> <output json>", file=sys.stderr)
        return 2
    log, out = Path(argv[0]), Path(argv[1])
    try:
        converted = events(log.read_text(encoding="utf-8", errors="replace"))
    except ConvertError as err:
        print(f"wasm_live: {err}", file=sys.stderr)
        return 1
    with out.open("w", encoding="utf-8") as sink:
        for event in converted:
            sink.write(json.dumps(event) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
