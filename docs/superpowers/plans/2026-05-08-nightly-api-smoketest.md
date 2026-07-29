# Nightly API Smoketest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two GitHub Actions workflows that run nightly at 06:00 UTC: one runs the no-auth `--ignored` live tests across all four crates and files an issue on real failure (with smart retries on transient infra failures); one fetches Polymarket's upstream OpenAPI specs and opens auto-PRs when they drift from the vendored copies in `docs/specs/`.

**Architecture:** Two workflows in `.github/workflows/` (`nightly-behavioral.yml`, `nightly-schema.yml`) plus two Python helper scripts in `.github/scripts/` (`classify_failures.py`, `diff_openapi.py`). Issue/PR mutations use the `gh` CLI directly in workflow YAML. Helpers are unit-tested with pytest, run on every CI build via a new `scripts` job in `ci.yml`.

**Tech Stack:** GitHub Actions (cron + workflow_dispatch), `gh` CLI, Python 3.11+ via `uv`, `pyyaml` (YAML canonicalization), `pytest` (script tests), `cargo nextest` with `--message-format libtest-json` (already in CI).

**Spec:** [`docs/superpowers/specs/2026-05-08-nightly-api-smoketest-design.md`](../specs/2026-05-08-nightly-api-smoketest-design.md)

---

## File Structure

**New files:**

| Path | Responsibility |
|------|----------------|
| `.github/scripts/pyproject.toml` | uv project for helper scripts; declares pyyaml + pytest as deps |
| `.github/scripts/classify_failures.py` | CLI: classifies nextest failures into auth-gated / transient / real, emits retry list and markdown report |
| `.github/scripts/diff_openapi.py` | CLI: fetches upstream OpenAPI, canonicalizes both sides, detects drift, emits markdown summary and copies upstream YAML to vendored path on drift |
| `.github/scripts/tests/__init__.py` | Empty — marks tests/ as a Python package |
| `.github/scripts/tests/test_classify_failures.py` | Pytest unit tests for classifier |
| `.github/scripts/tests/test_diff_openapi.py` | Pytest unit tests for OpenAPI diff |
| `.github/scripts/tests/fixtures/nextest-real-failure.json` | NDJSON fixture: one test panics with assertion failure |
| `.github/scripts/tests/fixtures/nextest-transient-429.json` | NDJSON fixture: one test fails with HTTP 429 |
| `.github/scripts/tests/fixtures/nextest-transient-503.json` | NDJSON fixture: one test fails with HTTP 503 |
| `.github/scripts/tests/fixtures/nextest-transient-connection.json` | NDJSON fixture: one test fails with connection refused |
| `.github/scripts/tests/fixtures/nextest-auth-gated.json` | NDJSON fixture: one test panics with POLYMARKET_* env-var helper message |
| `.github/scripts/tests/fixtures/nextest-mixed.json` | NDJSON fixture: 5 tests with all four outcomes (pass, real fail, transient, auth-gated) |
| `.github/scripts/tests/fixtures/openapi-no-drift/old.yaml` | Vendored OpenAPI |
| `.github/scripts/tests/fixtures/openapi-no-drift/new.yaml` | Identical to old.yaml (canonicalize-equivalent) |
| `.github/scripts/tests/fixtures/openapi-added-endpoint/old.yaml` | Vendored OpenAPI |
| `.github/scripts/tests/fixtures/openapi-added-endpoint/new.yaml` | Same with one new path added |
| `.github/scripts/tests/fixtures/openapi-removed-endpoint/old.yaml` | Vendored OpenAPI |
| `.github/scripts/tests/fixtures/openapi-removed-endpoint/new.yaml` | Same with one path removed |
| `.github/scripts/tests/fixtures/openapi-modified-schema/old.yaml` | Vendored OpenAPI |
| `.github/scripts/tests/fixtures/openapi-modified-schema/new.yaml` | Same with a response schema field added |
| `.github/workflows/nightly-behavioral.yml` | Cron + manual trigger, matrix per crate, run live tests, classify, retry transients, file/update/close issue |
| `.github/workflows/nightly-schema.yml` | Cron + manual trigger, matrix per crate, fetch upstream YAML, diff, open/update/close drift PR + tracking issue |

**Modified files:**

| Path | Change |
|------|--------|
| `.github/workflows/ci.yml` | Add `scripts` job that runs `pytest .github/scripts/tests/` on every PR |
| `CLAUDE.md` | Add a paragraph under "Testing Conventions" documenting the nightly workflows and how to enable auth tests later |

---

## Task 1: Bootstrap `.github/scripts/` Python project

**Files:**
- Create: `.github/scripts/pyproject.toml`
- Create: `.github/scripts/tests/__init__.py`
- Create: `.github/scripts/tests/test_smoke.py` (temporary — deleted at end of task)

- [ ] **Step 1: Create the pyproject.toml**

Write `.github/scripts/pyproject.toml`:

```toml
[project]
name = "polyoxide-ci-scripts"
version = "0.1.0"
description = "CI helper scripts for polyoxide nightly smoketest workflows"
requires-python = ">=3.11"
dependencies = [
    "pyyaml>=6.0",
]

[dependency-groups]
dev = [
    "pytest>=8.0",
]

[tool.pytest.ini_options]
testpaths = ["tests"]
python_files = ["test_*.py"]
```

- [ ] **Step 2: Create the empty package marker**

Write `.github/scripts/tests/__init__.py` with a single comment line:

```python
# Test package for polyoxide-ci-scripts.
```

- [ ] **Step 3: Create a temporary smoke test**

Write `.github/scripts/tests/test_smoke.py`:

```python
def test_pytest_runs():
    assert 1 + 1 == 2
```

- [ ] **Step 4: Verify uv sync + pytest runs**

Run from `.github/scripts/`:

```bash
cd .github/scripts && uv sync && uv run pytest -v
```

Expected output: 1 test passed (`test_smoke.py::test_pytest_runs PASSED`).

- [ ] **Step 5: Delete the smoke test**

```bash
rm .github/scripts/tests/test_smoke.py
```

- [ ] **Step 6: Commit**

```bash
git add .github/scripts/pyproject.toml .github/scripts/tests/__init__.py
git commit -m "chore(ci): bootstrap python project for nightly helper scripts"
```

---

## Task 2: nextest fixtures for classify_failures.py

**Files:**
- Create: `.github/scripts/tests/fixtures/nextest-real-failure.json`
- Create: `.github/scripts/tests/fixtures/nextest-transient-429.json`
- Create: `.github/scripts/tests/fixtures/nextest-transient-503.json`
- Create: `.github/scripts/tests/fixtures/nextest-transient-connection.json`
- Create: `.github/scripts/tests/fixtures/nextest-auth-gated.json`
- Create: `.github/scripts/tests/fixtures/nextest-mixed.json`

These fixtures use the libtest JSON format (NDJSON). Each line is a JSON event. Real failures include a `stdout` field with the panic output. The classifier inspects this `stdout` (concatenated with `stderr` if present) to assign a verdict.

> **Note for implementer:** if real `cargo nextest --message-format libtest-json` output diverges from these shapes (e.g., uses `message` instead of `stdout`, or wraps events in an envelope), update the fixtures and the parser in Task 4 together. Capture a real sample first by running any existing test:
> ```bash
> cargo nextest run -p polyoxide-gamma --test live_api --run-ignored only --message-format libtest-json 2>&1 | head -10
> ```

- [ ] **Step 1: Real failure fixture**

Write `.github/scripts/tests/fixtures/nextest-real-failure.json`:

```json
{ "type": "suite", "event": "started", "test_count": 1 }
{ "type": "test", "event": "started", "name": "live_list_markets" }
{ "type": "test", "name": "live_list_markets", "event": "failed", "stdout": "thread 'live_list_markets' panicked at polyoxide-gamma/tests/live_api.rs:42:5:\nassertion `left == right` failed\n  left: 0\n right: 100\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n" }
{ "type": "suite", "event": "failed", "passed": 0, "failed": 1, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 0.3 }
```

- [ ] **Step 2: Transient 429 fixture**

Write `.github/scripts/tests/fixtures/nextest-transient-429.json`:

```json
{ "type": "suite", "event": "started", "test_count": 1 }
{ "type": "test", "event": "started", "name": "live_list_markets" }
{ "type": "test", "name": "live_list_markets", "event": "failed", "stdout": "thread 'live_list_markets' panicked at polyoxide-gamma/tests/live_api.rs:25:5:\ngamma list markets: ApiError(Http { status: 429, message: \"Too Many Requests\" })\n" }
{ "type": "suite", "event": "failed", "passed": 0, "failed": 1, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 0.5 }
```

- [ ] **Step 3: Transient 503 fixture**

Write `.github/scripts/tests/fixtures/nextest-transient-503.json`:

```json
{ "type": "suite", "event": "started", "test_count": 1 }
{ "type": "test", "event": "started", "name": "live_get_market" }
{ "type": "test", "name": "live_get_market", "event": "failed", "stdout": "thread 'live_get_market' panicked at polyoxide-gamma/tests/live_api.rs:60:5:\ngamma get market: ApiError(Http { status: 503, message: \"Service Unavailable\" })\n" }
{ "type": "suite", "event": "failed", "passed": 0, "failed": 1, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 0.4 }
```

- [ ] **Step 4: Transient connection-error fixture**

Write `.github/scripts/tests/fixtures/nextest-transient-connection.json`:

```json
{ "type": "suite", "event": "started", "test_count": 1 }
{ "type": "test", "event": "started", "name": "live_ping" }
{ "type": "test", "name": "live_ping", "event": "failed", "stdout": "thread 'live_ping' panicked at polyoxide-gamma/tests/live_api.rs:12:5:\nping should succeed: ApiError(Network(\"Connection refused (os error 111)\"))\n" }
{ "type": "suite", "event": "failed", "passed": 0, "failed": 1, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 0.1 }
```

- [ ] **Step 5: Auth-gated fixture**

Write `.github/scripts/tests/fixtures/nextest-auth-gated.json`:

```json
{ "type": "suite", "event": "started", "test_count": 1 }
{ "type": "test", "event": "started", "name": "live_create_order" }
{ "type": "test", "name": "live_create_order", "event": "failed", "stdout": "thread 'live_create_order' panicked at polyoxide-clob/tests/live_api.rs:23:21:\nPOLYMARKET_* env vars required for authenticated tests: env error: NotPresent\n" }
{ "type": "suite", "event": "failed", "passed": 0, "failed": 1, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 0.0 }
```

- [ ] **Step 6: Mixed fixture (multiple tests, multiple outcomes)**

Write `.github/scripts/tests/fixtures/nextest-mixed.json`:

```json
{ "type": "suite", "event": "started", "test_count": 5 }
{ "type": "test", "event": "started", "name": "live_list_markets" }
{ "type": "test", "name": "live_list_markets", "event": "ok" }
{ "type": "test", "event": "started", "name": "live_get_market" }
{ "type": "test", "name": "live_get_market", "event": "failed", "stdout": "thread 'live_get_market' panicked at polyoxide-gamma/tests/live_api.rs:60:5:\nassertion failed: market.creator_address.is_some()\n" }
{ "type": "test", "event": "started", "name": "live_search_markets" }
{ "type": "test", "name": "live_search_markets", "event": "failed", "stdout": "thread 'live_search_markets' panicked at polyoxide-gamma/tests/live_api.rs:80:5:\nsearch markets: ApiError(Http { status: 429, message: \"Too Many Requests\" })\n" }
{ "type": "test", "event": "started", "name": "live_create_order" }
{ "type": "test", "name": "live_create_order", "event": "failed", "stdout": "thread 'live_create_order' panicked at polyoxide-clob/tests/live_api.rs:23:21:\nPOLYMARKET_* env vars required for authenticated tests: env error: NotPresent\n" }
{ "type": "test", "event": "started", "name": "live_get_order" }
{ "type": "test", "name": "live_get_order", "event": "failed", "stdout": "thread 'live_get_order' panicked at polyoxide-clob/tests/live_api.rs:130:5:\nget order: ApiError(Network(\"failed to lookup address: nodename nor servname provided\"))\n" }
{ "type": "suite", "event": "failed", "passed": 1, "failed": 4, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 1.2 }
```

- [ ] **Step 7: Commit**

```bash
git add .github/scripts/tests/fixtures/nextest-*.json
git commit -m "test(ci): add nextest output fixtures for classifier"
```

---

## Task 3: classify_failures.py — verdict classification (TDD)

**Files:**
- Create: `.github/scripts/classify_failures.py`
- Create: `.github/scripts/tests/test_classify_failures.py`

This task builds the core `classify(text) -> Verdict` function via three red-green cycles: REAL fall-through (trivial — proves harness), TRANSIENT pattern matching (whole transient regex set added together since they're one conceptual category), AUTH_GATED detection.

- [ ] **Step 1: Create skeleton with stub classify() that always returns REAL**

Write `.github/scripts/classify_failures.py`:

```python
#!/usr/bin/env python3
"""Classify cargo nextest failures into auth-gated / transient / real."""

from __future__ import annotations

import re
from enum import Enum


class Verdict(str, Enum):
    PASS = "pass"
    AUTH_GATED = "auth-gated"
    TRANSIENT = "transient"
    REAL = "real"


def classify(failure_output: str) -> Verdict:
    """Classify a single failure's combined stdout+stderr text."""
    return Verdict.REAL
```

- [ ] **Step 2: Write the test for REAL verdict (trivially passes against the stub)**

Write `.github/scripts/tests/test_classify_failures.py`:

```python
"""Unit tests for classify_failures.py."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from classify_failures import Verdict, classify

FIXTURES = Path(__file__).parent / "fixtures"


def _load_fixture_failure_text(name: str) -> str:
    """Read the first failed-event's stdout from an NDJSON fixture."""
    path = FIXTURES / name
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            event = json.loads(line)
            if event.get("type") == "test" and event.get("event") == "failed":
                return event.get("stdout", "") + event.get("stderr", "")
    raise AssertionError(f"no failed-event found in {name}")


def test_classify_real_assertion_failure() -> None:
    text = _load_fixture_failure_text("nextest-real-failure.json")
    assert classify(text) == Verdict.REAL
```

- [ ] **Step 3: Run the test, verify it passes (proves test harness works)**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 1 passed. (The stub returns REAL, fixture fails for assertion reasons, harness/import path works.)

- [ ] **Step 4: Add failing test for HTTP 429 → TRANSIENT**

Append to `tests/test_classify_failures.py`:

```python
def test_classify_transient_429() -> None:
    text = _load_fixture_failure_text("nextest-transient-429.json")
    assert classify(text) == Verdict.TRANSIENT
```

- [ ] **Step 5: Run, verify it fails (stub still returns REAL)**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py::test_classify_transient_429 -v
```

Expected: FAIL — `assert <Verdict.REAL: 'real'> == <Verdict.TRANSIENT: 'transient'>`.

- [ ] **Step 6: Add the full transient regex set and dispatch**

Modify `.github/scripts/classify_failures.py`. Add the patterns above the `classify` function, and update `classify` to consult them:

```python
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
    """Classify a single failure's combined stdout+stderr text."""
    for pat in TRANSIENT_RES:
        if pat.search(failure_output):
            return Verdict.TRANSIENT
    return Verdict.REAL
```

The transient patterns are added as a set rather than one-per-step because they all serve the same conceptual purpose (matching infrastructure-failure messages); fragmenting them into seven red-green cycles wouldn't yield better coverage.

- [ ] **Step 7: Run, verify the 429 test passes (and REAL still passes)**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 2 passed.

- [ ] **Step 8: Add tests covering the rest of the transient set**

Append to `tests/test_classify_failures.py`:

```python
def test_classify_transient_503() -> None:
    text = _load_fixture_failure_text("nextest-transient-503.json")
    assert classify(text) == Verdict.TRANSIENT


def test_classify_transient_connection_refused() -> None:
    text = _load_fixture_failure_text("nextest-transient-connection.json")
    assert classify(text) == Verdict.TRANSIENT
```

- [ ] **Step 9: Run, verify all 4 tests pass (regression coverage of transient set)**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 4 passed.

- [ ] **Step 10: Add failing test for AUTH_GATED**

Append to `tests/test_classify_failures.py`:

```python
def test_classify_auth_gated() -> None:
    text = _load_fixture_failure_text("nextest-auth-gated.json")
    assert classify(text) == Verdict.AUTH_GATED
```

- [ ] **Step 11: Run, verify it fails (current classifier has no auth path)**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py::test_classify_auth_gated -v
```

Expected: FAIL — auth-gated panic falls through to REAL.

- [ ] **Step 12: Add AUTH_GATED_RE check (with auth-takes-precedence ordering)**

Modify `.github/scripts/classify_failures.py`. Add the regex above the transient set, and check it first in `classify`:

```python
AUTH_GATED_RE = re.compile(r"POLYMARKET_\* env vars required", re.IGNORECASE)

# (TRANSIENT_RES list stays as-is)


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
```

- [ ] **Step 13: Run, verify all 5 tests pass**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 5 passed.

- [ ] **Step 14: Add edge-case tests for verdict ordering and ambiguity**

Append to `tests/test_classify_failures.py`:

```python
def test_classify_auth_gated_takes_precedence_over_transient() -> None:
    """If both patterns match, AUTH_GATED wins (defensive ordering)."""
    text = "POLYMARKET_* env vars required for authenticated tests: HTTP 503"
    assert classify(text) == Verdict.AUTH_GATED


def test_classify_empty_string_is_real() -> None:
    """No information defaults to REAL — better to false-positive than skip."""
    assert classify("") == Verdict.REAL


def test_classify_unrelated_5xx_substring_does_not_match() -> None:
    """The pattern requires `HTTP 5xx` or `status: 5xx`, not arbitrary 5xx."""
    assert classify("computed value 500 differs from expected 600") == Verdict.REAL
```

- [ ] **Step 15: Run, verify all 8 tests pass**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 8 passed.

- [ ] **Step 16: Commit**

```bash
git add .github/scripts/classify_failures.py .github/scripts/tests/test_classify_failures.py
git commit -m "feat(ci): classifier core for nextest failure categorization"
```

---

## Task 4: classify_failures.py — nextest JSON parser (TDD)

**Files:**
- Modify: `.github/scripts/classify_failures.py`
- Modify: `.github/scripts/tests/test_classify_failures.py`

This task adds parsing of full NDJSON files into a list of `(test_name, verdict)` tuples, including PASS handling. Mixed fixture exercises all four outcomes plus the parser's ability to handle multiple events.

- [ ] **Step 1: Add a failing test for the parser**

Append to `tests/test_classify_failures.py`:

```python
from classify_failures import TestOutcome, parse_nextest_json


def test_parse_mixed_fixture() -> None:
    outcomes = parse_nextest_json(FIXTURES / "nextest-mixed.json")
    by_name = {o.name: o for o in outcomes}
    assert by_name["live_list_markets"].verdict == Verdict.PASS
    assert by_name["live_get_market"].verdict == Verdict.REAL
    assert by_name["live_search_markets"].verdict == Verdict.TRANSIENT
    assert by_name["live_create_order"].verdict == Verdict.AUTH_GATED
    assert by_name["live_get_order"].verdict == Verdict.TRANSIENT
    assert len(outcomes) == 5
```

- [ ] **Step 2: Run, verify it fails (import error)**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py::test_parse_mixed_fixture -v
```

Expected: FAIL with `ImportError: cannot import name 'TestOutcome'` or `parse_nextest_json`.

- [ ] **Step 3: Implement the parser**

Append to `.github/scripts/classify_failures.py`:

```python
from dataclasses import dataclass
from pathlib import Path
import json


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
```

Note: Place the `from dataclasses import dataclass`, `from pathlib import Path`, and `import json` lines at the top of the file with the existing imports — don't append them inline.

- [ ] **Step 4: Run, verify the parser test passes**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py::test_parse_mixed_fixture -v
```

Expected: PASS.

- [ ] **Step 5: Run all tests in the file, verify still 9 pass total**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 9 passed.

- [ ] **Step 6: Commit**

```bash
git add .github/scripts/classify_failures.py .github/scripts/tests/test_classify_failures.py
git commit -m "feat(ci): nextest NDJSON parser in classify_failures.py"
```

---

## Task 5: classify_failures.py — CLI commands (classify and merge)

**Files:**
- Modify: `.github/scripts/classify_failures.py`
- Modify: `.github/scripts/tests/test_classify_failures.py`

This task wires up the CLI. Two subcommands:

- `classify --input nextest.json --output-dir DIR` — first-pass classification
- `merge --first-pass first.json --retry retry.json --output-dir DIR` — merges first-pass + retry results

Both write four files into `--output-dir`:
- `retry-tests.txt` — newline-separated test names with TRANSIENT verdict (consumed only from `classify`, ignored in `merge`)
- `real-failures.txt` — newline-separated test names with REAL verdict
- `auth-gated.txt` — newline-separated test names with AUTH_GATED verdict (informational)
- `report.md` — markdown report. Empty-ish (just a "no real failures" line) when no real failures exist.

The aggregator workflow uses `report.md` as the issue body and `real-failures.txt`'s emptiness as the "should I file an issue" signal.

- [ ] **Step 1: Add failing test for classify subcommand**

Append to `tests/test_classify_failures.py`:

```python
import subprocess
import sys


SCRIPT = Path(__file__).parent.parent / "classify_failures.py"


def test_cli_classify_writes_outputs(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "classify",
            "--input", str(FIXTURES / "nextest-mixed.json"),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.returncode == 0

    retry = (out_dir / "retry-tests.txt").read_text().splitlines()
    real = (out_dir / "real-failures.txt").read_text().splitlines()
    auth = (out_dir / "auth-gated.txt").read_text().splitlines()
    report = (out_dir / "report.md").read_text()

    # mixed.json has: 1 pass, 1 real, 1 transient (429), 1 auth-gated, 1 transient (DNS)
    assert sorted(retry) == sorted(["live_search_markets", "live_get_order"])
    assert real == ["live_get_market"]
    assert auth == ["live_create_order"]
    assert "live_get_market" in report
    assert "## Real failures" in report
```

- [ ] **Step 2: Run, verify it fails**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py::test_cli_classify_writes_outputs -v
```

Expected: FAIL — script has no main entry point yet.

- [ ] **Step 3: Implement the CLI**

Append to `.github/scripts/classify_failures.py`:

```python
import argparse
import sys


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
```

- [ ] **Step 4: Run, verify the classify CLI test passes**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py::test_cli_classify_writes_outputs -v
```

Expected: PASS.

- [ ] **Step 5: Add failing test for merge subcommand**

Append to `tests/test_classify_failures.py`:

```python
def test_cli_merge_promotes_persistent_transients_to_real(tmp_path: Path) -> None:
    """A test that was transient on first pass and still failing on retry
    becomes a REAL failure in the merged report."""
    # Reuse mixed.json as first-pass: live_search_markets is TRANSIENT.
    # Use a synthetic retry that has live_search_markets STILL failing transient.
    retry_file = tmp_path / "retry.json"
    retry_file.write_text(
        '{"type":"suite","event":"started","test_count":2}\n'
        '{"type":"test","event":"started","name":"live_search_markets"}\n'
        '{"type":"test","name":"live_search_markets","event":"failed","stdout":"thread panic: HTTP 429"}\n'
        '{"type":"test","event":"started","name":"live_get_order"}\n'
        '{"type":"test","name":"live_get_order","event":"ok"}\n'
        '{"type":"suite","event":"failed","passed":1,"failed":1,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.5}\n'
    )

    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "merge",
            "--first-pass", str(FIXTURES / "nextest-mixed.json"),
            "--retry", str(retry_file),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.returncode == 0

    real = (out_dir / "real-failures.txt").read_text().splitlines()
    # live_get_market was REAL on first pass — stays REAL.
    # live_search_markets was TRANSIENT on first pass, still failing on retry — promoted to REAL.
    # live_get_order was TRANSIENT on first pass, passed on retry — drops to PASS.
    assert sorted(real) == sorted(["live_get_market", "live_search_markets"])
    assert (out_dir / "retry-tests.txt").read_text() == ""
```

- [ ] **Step 6: Run, verify pass**

```bash
cd .github/scripts && uv run pytest tests/test_classify_failures.py -v
```

Expected: 11 passed.

- [ ] **Step 7: Commit**

```bash
git add .github/scripts/classify_failures.py .github/scripts/tests/test_classify_failures.py
git commit -m "feat(ci): CLI for classify_failures.py with classify and merge subcommands"
```

---

## Task 6: OpenAPI fixtures for diff_openapi.py

**Files:**
- Create: `.github/scripts/tests/fixtures/openapi-no-drift/old.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-no-drift/new.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-added-endpoint/old.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-added-endpoint/new.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-removed-endpoint/old.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-removed-endpoint/new.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-modified-schema/old.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-modified-schema/new.yaml`

These minimal OpenAPI 3.0 specs exercise the four drift scenarios. They're intentionally small: a couple of endpoints, simple schemas. The diff logic should be schema-agnostic enough that small specs cover it.

- [ ] **Step 1: no-drift pair (identical content, different formatting)**

Write `.github/scripts/tests/fixtures/openapi-no-drift/old.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
```

Write `.github/scripts/tests/fixtures/openapi-no-drift/new.yaml` — same content but with reordered keys to test canonicalization:

```yaml
openapi: 3.0.0
paths:
  /markets:
    get:
      responses:
        '200':
          description: OK
      summary: List markets
info:
  version: 1.0.0
  title: Test API
```

- [ ] **Step 2: added-endpoint pair**

Write `.github/scripts/tests/fixtures/openapi-added-endpoint/old.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
```

Write `.github/scripts/tests/fixtures/openapi-added-endpoint/new.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
  /markets/{id}:
    get:
      summary: Get a single market by id
      responses:
        '200':
          description: OK
```

- [ ] **Step 3: removed-endpoint pair**

Write `.github/scripts/tests/fixtures/openapi-removed-endpoint/old.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
  /events:
    get:
      summary: List events
      responses:
        '200':
          description: OK
```

Write `.github/scripts/tests/fixtures/openapi-removed-endpoint/new.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
```

- [ ] **Step 4: modified-schema pair**

Write `.github/scripts/tests/fixtures/openapi-modified-schema/old.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    type: string
                  question:
                    type: string
```

Write `.github/scripts/tests/fixtures/openapi-modified-schema/new.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /markets:
    get:
      summary: List markets
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    type: string
                  question:
                    type: string
                  creator_address:
                    type: string
```

- [ ] **Step 5: Commit**

```bash
git add .github/scripts/tests/fixtures/openapi-*
git commit -m "test(ci): add OpenAPI drift fixtures"
```

---

## Task 7: diff_openapi.py — canonicalization (TDD)

**Files:**
- Create: `.github/scripts/diff_openapi.py`
- Create: `.github/scripts/tests/test_diff_openapi.py`

This task builds `canonicalize(yaml_text) -> str`, which loads YAML and re-emits it as JSON with sorted keys. Tests verify that semantically-equivalent specs produce equal canonical forms.

- [ ] **Step 1: Create the test file with a failing test**

Write `.github/scripts/tests/test_diff_openapi.py`:

```python
"""Unit tests for diff_openapi.py."""

from __future__ import annotations

from pathlib import Path

from diff_openapi import canonicalize

FIXTURES = Path(__file__).parent / "fixtures"


def test_canonicalize_makes_reordered_yaml_equal() -> None:
    old = (FIXTURES / "openapi-no-drift" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-no-drift" / "new.yaml").read_text()
    assert canonicalize(old) == canonicalize(new)


def test_canonicalize_distinguishes_added_endpoint() -> None:
    old = (FIXTURES / "openapi-added-endpoint" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-added-endpoint" / "new.yaml").read_text()
    assert canonicalize(old) != canonicalize(new)
```

- [ ] **Step 2: Run, verify it fails (no module)**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'diff_openapi'`.

- [ ] **Step 3: Implement canonicalize()**

Write `.github/scripts/diff_openapi.py`:

```python
#!/usr/bin/env python3
"""Detect OpenAPI schema drift between Polymarket upstream and our vendored copies."""

from __future__ import annotations

import json

import yaml


def canonicalize(yaml_text: str) -> str:
    """Return a canonical string for the YAML's structural content.

    Comments, key ordering, anchors, and YAML-specific syntax are erased:
    we parse to a Python value tree and emit JSON with sorted keys. Two
    YAMLs whose structural content matches will produce identical strings.
    """
    parsed = yaml.safe_load(yaml_text)
    return json.dumps(parsed, sort_keys=True, indent=2)
```

- [ ] **Step 4: Run, verify both tests pass**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): canonicalize() for OpenAPI YAML drift comparison"
```

---

## Task 8: diff_openapi.py — drift detection and structured summary (TDD)

**Files:**
- Modify: `.github/scripts/diff_openapi.py`
- Modify: `.github/scripts/tests/test_diff_openapi.py`

This task adds `detect_drift(old_yaml, new_yaml) -> DriftResult`. The result includes:
- `has_drift: bool`
- `endpoints_added: list[str]` (e.g., `["GET /markets/{id}"]`)
- `endpoints_removed: list[str]`
- `endpoints_modified: list[str]`

Implementation walks the parsed `paths` dict directly and compares per-method bodies. (The spec mentioned `deepdiff` as a candidate; plain dict iteration turns out to be cleaner and removes a dependency.)

- [ ] **Step 1: Failing test for no-drift case**

Append to `tests/test_diff_openapi.py`:

```python
from diff_openapi import detect_drift


def test_detect_drift_no_drift_returns_clean() -> None:
    old = (FIXTURES / "openapi-no-drift" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-no-drift" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is False
    assert result.endpoints_added == []
    assert result.endpoints_removed == []
    assert result.endpoints_modified == []
```

- [ ] **Step 2: Run, verify fail (no detect_drift)**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: FAIL.

- [ ] **Step 3: Implement minimal detect_drift (boolean only, empty lists)**

Append to `.github/scripts/diff_openapi.py`:

```python
from dataclasses import dataclass, field


@dataclass
class DriftResult:
    has_drift: bool
    endpoints_added: list[str] = field(default_factory=list)
    endpoints_removed: list[str] = field(default_factory=list)
    endpoints_modified: list[str] = field(default_factory=list)


def detect_drift(old_yaml: str, new_yaml: str) -> DriftResult:
    """Compare two OpenAPI YAML strings and report endpoint-level changes."""
    if canonicalize(old_yaml) == canonicalize(new_yaml):
        return DriftResult(has_drift=False)
    return DriftResult(has_drift=True)
```

Place `from dataclasses import dataclass, field` at the top of the file with the other imports.

- [ ] **Step 4: Run, verify the no-drift test passes**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py::test_detect_drift_no_drift_returns_clean -v
```

Expected: PASS.

- [ ] **Step 5: Add failing tests for added/removed/modified**

Append to `tests/test_diff_openapi.py`:

```python
def test_detect_drift_added_endpoint() -> None:
    old = (FIXTURES / "openapi-added-endpoint" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-added-endpoint" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    assert result.endpoints_added == ["GET /markets/{id}"]
    assert result.endpoints_removed == []
    assert result.endpoints_modified == []


def test_detect_drift_removed_endpoint() -> None:
    old = (FIXTURES / "openapi-removed-endpoint" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-removed-endpoint" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    assert result.endpoints_added == []
    assert result.endpoints_removed == ["GET /events"]
    assert result.endpoints_modified == []


def test_detect_drift_modified_schema() -> None:
    old = (FIXTURES / "openapi-modified-schema" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-modified-schema" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    assert result.endpoints_added == []
    assert result.endpoints_removed == []
    assert result.endpoints_modified == ["GET /markets"]
```

- [ ] **Step 6: Run, verify the three new tests fail (lists are still empty)**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: 2 passed (canonicalize tests + no-drift), 3 failed — all asserting on empty lists rather than expected endpoint identifiers.

- [ ] **Step 7: Implement path-walking logic to populate the lists**

Replace the body of `detect_drift` in `.github/scripts/diff_openapi.py` with:

```python
def detect_drift(old_yaml: str, new_yaml: str) -> DriftResult:
    """Compare two OpenAPI YAML strings and report endpoint-level changes."""
    if canonicalize(old_yaml) == canonicalize(new_yaml):
        return DriftResult(has_drift=False)

    old_doc = yaml.safe_load(old_yaml) or {}
    new_doc = yaml.safe_load(new_yaml) or {}
    old_paths = old_doc.get("paths") or {}
    new_paths = new_doc.get("paths") or {}

    added: list[str] = []
    removed: list[str] = []
    modified: list[str] = []

    for path, ops in new_paths.items():
        if path not in old_paths:
            for method in ops:
                added.append(f"{method.upper()} {path}")
        else:
            for method, body in ops.items():
                if method not in (old_paths[path] or {}):
                    added.append(f"{method.upper()} {path}")
                elif old_paths[path][method] != body:
                    modified.append(f"{method.upper()} {path}")

    for path, ops in old_paths.items():
        if path not in new_paths:
            for method in ops:
                removed.append(f"{method.upper()} {path}")
        else:
            for method in ops:
                if method not in (new_paths[path] or {}):
                    removed.append(f"{method.upper()} {path}")

    return DriftResult(
        has_drift=True,
        endpoints_added=sorted(added),
        endpoints_removed=sorted(removed),
        endpoints_modified=sorted(modified),
    )
```

- [ ] **Step 8: Run, verify all 5 tests pass**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: 5 passed.

- [ ] **Step 9: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): structural drift detection for OpenAPI specs"
```

---

## Task 9: diff_openapi.py — markdown summary + CLI (TDD)

**Files:**
- Modify: `.github/scripts/diff_openapi.py`
- Modify: `.github/scripts/tests/test_diff_openapi.py`

The CLI takes a fetched upstream YAML path and a vendored YAML path, runs `detect_drift`, and emits to `--output-dir`:
- `summary.md` — markdown report (always, even when no drift, for predictable downstream parsing)
- `unified-diff.txt` — a unified diff between canonical forms (only on drift)
- A side-effect: if `--apply-on-drift` flag is set AND drift is detected, the upstream raw bytes are copied to the vendored path

Exit code: 0 if no drift, 1 if drift detected, 2 if a parse error occurred.

- [ ] **Step 1: Failing test for summary rendering**

Append to `tests/test_diff_openapi.py`:

```python
import subprocess
import sys

from diff_openapi import render_summary


SCRIPT = Path(__file__).parent.parent / "diff_openapi.py"


def test_render_summary_no_drift() -> None:
    from diff_openapi import DriftResult

    text = render_summary(DriftResult(has_drift=False), crate="clob", upstream_url="https://x")
    assert "No drift detected" in text


def test_render_summary_with_changes() -> None:
    from diff_openapi import DriftResult

    result = DriftResult(
        has_drift=True,
        endpoints_added=["POST /new"],
        endpoints_removed=["GET /old"],
        endpoints_modified=["GET /markets"],
    )
    text = render_summary(result, crate="clob", upstream_url="https://docs.polymarket.com/api-spec/clob-openapi.yaml")
    assert "## Endpoints added" in text
    assert "POST /new" in text
    assert "## Endpoints removed" in text
    assert "GET /old" in text
    assert "## Endpoints modified" in text
    assert "GET /markets" in text
    assert "https://docs.polymarket.com/api-spec/clob-openapi.yaml" in text
```

- [ ] **Step 2: Run, verify fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: FAIL on the two new tests (`render_summary` not defined).

- [ ] **Step 3: Implement render_summary()**

Append to `.github/scripts/diff_openapi.py`:

```python
def render_summary(result: DriftResult, crate: str, upstream_url: str) -> str:
    if not result.has_drift:
        return f"No drift detected for `{crate}` against `{upstream_url}`.\n"

    lines = [
        f"## Schema drift in `{crate}`",
        "",
        f"Upstream OpenAPI at <{upstream_url}> differs from vendored `docs/specs/{crate}/openapi.yaml`.",
        "",
    ]
    if result.endpoints_added:
        lines.append("## Endpoints added")
        lines.append("")
        for ep in result.endpoints_added:
            lines.append(f"- `{ep}`")
        lines.append("")
    if result.endpoints_removed:
        lines.append("## Endpoints removed")
        lines.append("")
        for ep in result.endpoints_removed:
            lines.append(f"- `{ep}`")
        lines.append("")
    if result.endpoints_modified:
        lines.append("## Endpoints modified")
        lines.append("")
        for ep in result.endpoints_modified:
            lines.append(f"- `{ep}`")
        lines.append("")
    return "\n".join(lines)
```

- [ ] **Step 4: Run, verify pass**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: 7 passed.

- [ ] **Step 5: Failing test for the CLI**

Append to `tests/test_diff_openapi.py`:

```python
def test_cli_check_no_drift_exits_zero(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "test",
            "--upstream-yaml", str(FIXTURES / "openapi-no-drift" / "old.yaml"),
            "--vendored-yaml", str(FIXTURES / "openapi-no-drift" / "new.yaml"),
            "--upstream-url", "https://example.com/test.yaml",
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    summary = (out_dir / "summary.md").read_text()
    assert "No drift detected" in summary
    assert not (out_dir / "unified-diff.txt").exists()


def test_cli_check_with_drift_exits_one_and_writes_diff(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "test",
            "--upstream-yaml", str(FIXTURES / "openapi-added-endpoint" / "new.yaml"),
            "--vendored-yaml", str(FIXTURES / "openapi-added-endpoint" / "old.yaml"),
            "--upstream-url", "https://example.com/test.yaml",
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    summary = (out_dir / "summary.md").read_text()
    assert "GET /markets/{id}" in summary
    assert (out_dir / "unified-diff.txt").exists()


def test_cli_check_apply_on_drift_copies_upstream_to_vendored(tmp_path: Path) -> None:
    upstream_src = FIXTURES / "openapi-added-endpoint" / "new.yaml"
    vendored = tmp_path / "vendored.yaml"
    vendored.write_text((FIXTURES / "openapi-added-endpoint" / "old.yaml").read_text())
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "test",
            "--upstream-yaml", str(upstream_src),
            "--vendored-yaml", str(vendored),
            "--upstream-url", "https://example.com/test.yaml",
            "--output-dir", str(out_dir),
            "--apply-on-drift",
        ],
        capture_output=True,
        text=True,
    )
    # Vendored file now equals upstream, byte for byte.
    assert vendored.read_bytes() == upstream_src.read_bytes()
```

- [ ] **Step 6: Run, verify fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: FAIL on the three CLI tests.

- [ ] **Step 7: Implement the CLI**

Append to `.github/scripts/diff_openapi.py`:

```python
import argparse
import difflib
import shutil
import sys
from pathlib import Path


def _cmd_check(args: argparse.Namespace) -> int:
    upstream_yaml = args.upstream_yaml.read_text()
    vendored_yaml = args.vendored_yaml.read_text()
    try:
        result = detect_drift(vendored_yaml, upstream_yaml)
    except yaml.YAMLError as exc:
        print(f"YAML parse error: {exc}", file=sys.stderr)
        return 2

    args.output_dir.mkdir(parents=True, exist_ok=True)
    summary = render_summary(result, crate=args.crate, upstream_url=args.upstream_url)
    (args.output_dir / "summary.md").write_text(summary)

    if not result.has_drift:
        return 0

    canonical_old = canonicalize(vendored_yaml).splitlines(keepends=True)
    canonical_new = canonicalize(upstream_yaml).splitlines(keepends=True)
    diff = "".join(difflib.unified_diff(
        canonical_old, canonical_new,
        fromfile=f"vendored/{args.crate}/openapi.yaml (canonical)",
        tofile=f"upstream/{args.crate}/openapi.yaml (canonical)",
    ))
    (args.output_dir / "unified-diff.txt").write_text(diff)

    if args.apply_on_drift:
        shutil.copyfile(args.upstream_yaml, args.vendored_yaml)

    return 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Detect OpenAPI schema drift.")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("check", help="Compare upstream vs vendored OpenAPI.")
    p.add_argument("--crate", required=True, help="e.g. clob, gamma, data, relay")
    p.add_argument("--upstream-yaml", type=Path, required=True, help="Path to fetched upstream YAML")
    p.add_argument("--vendored-yaml", type=Path, required=True, help="Path to docs/specs/<crate>/openapi.yaml")
    p.add_argument("--upstream-url", required=True, help="URL the upstream was fetched from (for the summary)")
    p.add_argument("--output-dir", type=Path, required=True)
    p.add_argument("--apply-on-drift", action="store_true",
                   help="If drift detected, overwrite vendored-yaml with upstream-yaml's raw bytes")
    p.set_defaults(func=_cmd_check)

    ns = parser.parse_args(argv)
    return ns.func(ns)


if __name__ == "__main__":
    sys.exit(main())
```

Place `import argparse`, `import difflib`, `import shutil`, `import sys`, and `from pathlib import Path` at the top with other imports.

- [ ] **Step 8: Run, verify all 10 tests pass**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -v
```

Expected: 10 passed.

- [ ] **Step 9: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): CLI for diff_openapi.py with check subcommand"
```

---

## Task 10: Add `scripts` job to ci.yml

**Files:**
- Modify: `.github/workflows/ci.yml`

This adds a CI job that runs the helper-script unit tests on every PR, so script bugs are caught before nightly fires.

- [ ] **Step 1: Read the current ci.yml**

```bash
cat .github/workflows/ci.yml
```

- [ ] **Step 2: Add the scripts job**

Edit `.github/workflows/ci.yml`. After the existing `python:` job (and before EOF), add:

```yaml

  scripts:
    name: CI Scripts
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v6
      - run: uv sync
        working-directory: .github/scripts
      - run: uv run pytest tests/ -v
        working-directory: .github/scripts
```

- [ ] **Step 3: Verify locally that the scripts pass tests**

```bash
cd .github/scripts && uv sync && uv run pytest tests/ -v && cd -
```

Expected: 22 passed (11 in test_classify_failures.py + 11 in test_diff_openapi.py — recount during execution if step counts have changed).

- [ ] **Step 4: Verify YAML syntax is valid**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"
```

Expected: no output (parse succeeds).

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run helper script unit tests on every PR"
```

---

## Task 11: nightly-behavioral.yml

**Files:**
- Create: `.github/workflows/nightly-behavioral.yml`

The full workflow with matrix, classify, conditional retry, aggregator, and issue management.

- [ ] **Step 1: Write the workflow**

Write `.github/workflows/nightly-behavioral.yml`:

```yaml
name: Nightly behavioral smoketest

on:
  schedule:
    - cron: "0 6 * * *"
  workflow_dispatch: {}

permissions:
  contents: read
  issues: write

concurrency:
  group: nightly-behavioral
  cancel-in-progress: false

env:
  CARGO_INCREMENTAL: 0
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test:
    name: Live tests (${{ matrix.crate }})
    runs-on: ubuntu-latest
    timeout-minutes: 15
    strategy:
      fail-fast: false
      matrix:
        crate: [polyoxide-gamma, polyoxide-data, polyoxide-clob, polyoxide-relay]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@nextest
      - uses: astral-sh/setup-uv@v6
      - run: uv sync
        working-directory: .github/scripts

      - name: Run live (no-auth) tests — first pass
        run: |
          mkdir -p artifacts/${{ matrix.crate }}
          cargo nextest run \
            -p ${{ matrix.crate }} \
            --test live_api \
            --run-ignored only \
            --no-fail-fast \
            --message-format libtest-json \
            > artifacts/${{ matrix.crate }}/first-pass.json 2>artifacts/${{ matrix.crate }}/first-pass.stderr || true

      - name: Classify first-pass failures
        run: |
          uv run python ${{ github.workspace }}/.github/scripts/classify_failures.py classify \
            --input artifacts/${{ matrix.crate }}/first-pass.json \
            --output-dir artifacts/${{ matrix.crate }}/classified
        working-directory: .github/scripts

      - name: Retry transient failures
        run: |
          RETRY_LIST="artifacts/${{ matrix.crate }}/classified/retry-tests.txt"
          if [ -s "$RETRY_LIST" ]; then
            FILTER=$(awk 'BEGIN{first=1} {if(first==1){printf "test(=%s)", $0; first=0} else {printf " | test(=%s)", $0}}' "$RETRY_LIST")
            cargo nextest run \
              -p ${{ matrix.crate }} \
              --test live_api \
              --run-ignored only \
              --no-fail-fast \
              --message-format libtest-json \
              --retries 2 \
              -E "$FILTER" \
              > artifacts/${{ matrix.crate }}/retry.json 2>artifacts/${{ matrix.crate }}/retry.stderr || true
          else
            echo '{"type":"suite","event":"started","test_count":0}' > artifacts/${{ matrix.crate }}/retry.json
            echo '{"type":"suite","event":"ok","passed":0,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.0}' >> artifacts/${{ matrix.crate }}/retry.json
          fi

      - name: Merge first-pass + retry
        run: |
          uv run python ${{ github.workspace }}/.github/scripts/classify_failures.py merge \
            --first-pass ${{ github.workspace }}/artifacts/${{ matrix.crate }}/first-pass.json \
            --retry ${{ github.workspace }}/artifacts/${{ matrix.crate }}/retry.json \
            --output-dir ${{ github.workspace }}/artifacts/${{ matrix.crate }}/merged
        working-directory: .github/scripts

      - name: Upload artifacts
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: behavioral-${{ matrix.crate }}
          path: artifacts/${{ matrix.crate }}/

  aggregate:
    name: Aggregate and report
    needs: [test]
    if: always()
    runs-on: ubuntu-latest
    permissions:
      contents: read
      issues: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          path: all-artifacts/
          pattern: behavioral-*

      - name: Build combined report
        id: report
        run: |
          REPORT_FILE=$(mktemp)
          ANY_REAL_FAILURE="false"
          echo "# Nightly behavioral check — $(date -u +%Y-%m-%d)" > "$REPORT_FILE"
          echo "" >> "$REPORT_FILE"
          for d in all-artifacts/behavioral-*; do
            crate=$(basename "$d" | sed 's/^behavioral-//')
            real_failures="$d/merged/real-failures.txt"
            report_md="$d/merged/report.md"
            if [ -s "$real_failures" ]; then
              ANY_REAL_FAILURE="true"
              echo "## ${crate}" >> "$REPORT_FILE"
              echo "" >> "$REPORT_FILE"
              cat "$report_md" >> "$REPORT_FILE"
              echo "" >> "$REPORT_FILE"
            fi
          done
          echo "any_real_failure=$ANY_REAL_FAILURE" >> "$GITHUB_OUTPUT"
          echo "report_file=$REPORT_FILE" >> "$GITHUB_OUTPUT"

      - name: File or update tracking issue
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          EXISTING=$(gh issue list --label nightly-behavioral --state open --json number --jq '.[0].number // empty')
          if [ "${{ steps.report.outputs.any_real_failure }}" = "true" ]; then
            BODY="$(cat ${{ steps.report.outputs.report_file }})"
            if [ -n "$EXISTING" ]; then
              gh issue comment "$EXISTING" --body "$BODY"
            else
              gh issue create \
                --title "Nightly behavioral check failed: $(date -u +%Y-%m-%d)" \
                --label nightly-behavioral \
                --body "$BODY"
            fi
          elif [ -n "$EXISTING" ]; then
            gh issue close "$EXISTING" --comment "Recovered $(date -u +%Y-%m-%d)"
          fi
```

- [ ] **Step 2: Verify YAML syntax is valid**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/nightly-behavioral.yml'))"
```

Expected: no output (parse succeeds).

- [ ] **Step 3: Verify the nextest filter expression syntax (sanity)**

The retry step builds a nextest filter expression like `test(=name1) | test(=name2)`. Confirm by checking that `awk` produces the expected shape on a fake input:

```bash
echo -e "test_a\ntest_b\ntest_c" | awk 'BEGIN{first=1} {if(first==1){printf "test(=%s)", $0; first=0} else {printf " | test(=%s)", $0}}'
```

Expected output: `test(=test_a) | test(=test_b) | test(=test_c)`

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/nightly-behavioral.yml
git commit -m "ci: nightly behavioral smoketest workflow against live Polymarket APIs"
```

---

## Task 12: nightly-schema.yml

**Files:**
- Create: `.github/workflows/nightly-schema.yml`

This workflow fetches each crate's upstream OpenAPI YAML, runs `diff_openapi.py check --apply-on-drift`, and on drift opens or updates a deterministic-branch PR plus a tracking issue.

- [ ] **Step 1: Write the workflow**

Write `.github/workflows/nightly-schema.yml`:

```yaml
name: Nightly schema drift

on:
  schedule:
    - cron: "0 6 * * *"
  workflow_dispatch: {}

permissions:
  contents: write
  pull-requests: write
  issues: write

concurrency:
  group: nightly-schema
  cancel-in-progress: false

jobs:
  check:
    name: Check ${{ matrix.crate }}
    runs-on: ubuntu-latest
    timeout-minutes: 10
    strategy:
      fail-fast: false
      matrix:
        include:
          - { crate: clob,  upstream: clob-openapi.yaml }
          - { crate: gamma, upstream: gamma-openapi.yaml }
          - { crate: data,  upstream: data-openapi.yaml }
          - { crate: relay, upstream: relayer-openapi.yaml }
    env:
      UPSTREAM_BASE: https://docs.polymarket.com/api-spec
      BRANCH: nightly-schema-drift/${{ matrix.crate }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: astral-sh/setup-uv@v6
      - run: uv sync
        working-directory: .github/scripts

      - name: Configure git for bot commits
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

      - name: Fetch upstream OpenAPI (with retries)
        id: fetch
        run: |
          UPSTREAM_URL="${UPSTREAM_BASE}/${{ matrix.upstream }}"
          UPSTREAM_FILE=$(mktemp --suffix=.yaml)
          echo "url=$UPSTREAM_URL" >> "$GITHUB_OUTPUT"
          echo "file=$UPSTREAM_FILE" >> "$GITHUB_OUTPUT"
          # 3 retries, 5s initial backoff, doubling each time.
          for delay in 0 5 10 20; do
            sleep "$delay"
            if curl -fsSL --max-time 30 -o "$UPSTREAM_FILE" "$UPSTREAM_URL"; then
              echo "fetched=true" >> "$GITHUB_OUTPUT"
              exit 0
            fi
          done
          echo "fetched=false" >> "$GITHUB_OUTPUT"
          echo "::warning::Failed to fetch ${UPSTREAM_URL} after retries; skipping drift check for this crate."
          # Exit success so the matrix entry doesn't count as "infra failure" — we
          # explicitly choose to skip rather than file false-positive PRs.
          exit 0

      - name: Detect drift
        id: drift
        if: steps.fetch.outputs.fetched == 'true'
        run: |
          set +e
          uv run python ${{ github.workspace }}/.github/scripts/diff_openapi.py check \
            --crate ${{ matrix.crate }} \
            --upstream-yaml "${{ steps.fetch.outputs.file }}" \
            --vendored-yaml ${{ github.workspace }}/docs/specs/${{ matrix.crate }}/openapi.yaml \
            --upstream-url "${{ steps.fetch.outputs.url }}" \
            --output-dir ${{ github.workspace }}/artifacts/${{ matrix.crate }} \
            --apply-on-drift
          ec=$?
          set -e
          if [ "$ec" -eq 0 ]; then
            echo "drift=false" >> "$GITHUB_OUTPUT"
          elif [ "$ec" -eq 1 ]; then
            echo "drift=true" >> "$GITHUB_OUTPUT"
          else
            echo "::error::diff_openapi.py exited with $ec — see logs"
            exit "$ec"
          fi
        working-directory: .github/scripts

      - name: Open or update drift PR + issue
        if: steps.fetch.outputs.fetched == 'true' && steps.drift.outputs.drift == 'true'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          BRANCH: ${{ env.BRANCH }}
        run: |
          ARTIFACT_DIR="${{ github.workspace }}/artifacts/${{ matrix.crate }}"

          # Re-checkout the schema-drift branch (or create it).
          git fetch origin "${BRANCH}:${BRANCH}" 2>/dev/null || true
          git checkout -B "${BRANCH}"
          git add docs/specs/${{ matrix.crate }}/openapi.yaml
          if git diff --staged --quiet; then
            echo "No file change to commit (perhaps idempotent re-run); aborting PR ops."
            exit 0
          fi
          git commit -m "chore(specs): sync ${{ matrix.crate }} OpenAPI from upstream"
          git push --force-with-lease origin "${BRANCH}"

          ISSUE_TITLE="Schema drift: ${{ matrix.crate }}"
          EXISTING_ISSUE=$(gh issue list --label schema-drift --search "${ISSUE_TITLE} in:title" --state open --json number --jq '.[0].number // empty')
          if [ -z "$EXISTING_ISSUE" ]; then
            EXISTING_ISSUE=$(gh issue create --title "${ISSUE_TITLE}" --label schema-drift --body "$(cat ${ARTIFACT_DIR}/summary.md)" | awk -F/ '{print $NF}')
          else
            gh issue edit "$EXISTING_ISSUE" --body "$(cat ${ARTIFACT_DIR}/summary.md)"
          fi

          PR_BODY=$(mktemp)
          {
            cat "${ARTIFACT_DIR}/summary.md"
            echo ""
            echo "<details><summary>Full canonicalized diff</summary>"
            echo ""
            echo '```diff'
            cat "${ARTIFACT_DIR}/unified-diff.txt"
            echo '```'
            echo "</details>"
            echo ""
            echo "Closes #${EXISTING_ISSUE}"
          } > "$PR_BODY"

          EXISTING_PR=$(gh pr list --head "${BRANCH}" --state open --json number --jq '.[0].number // empty')
          if [ -z "$EXISTING_PR" ]; then
            gh pr create \
              --title "chore(specs): ${{ matrix.crate }} drift on $(date -u +%Y-%m-%d)" \
              --body "$(cat ${PR_BODY})" \
              --label schema-drift \
              --head "${BRANCH}" \
              --base main
          else
            gh pr edit "$EXISTING_PR" --body "$(cat ${PR_BODY})"
          fi

      - name: Close stale drift PR + issue (recovery)
        if: steps.fetch.outputs.fetched == 'true' && steps.drift.outputs.drift == 'false'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          BRANCH: ${{ env.BRANCH }}
        run: |
          EXISTING_PR=$(gh pr list --head "${BRANCH}" --state open --json number --jq '.[0].number // empty')
          if [ -n "$EXISTING_PR" ]; then
            gh pr close "$EXISTING_PR" --comment "Drift no longer present (upstream reverted or vendored already updated)"
          fi
          ISSUE_TITLE="Schema drift: ${{ matrix.crate }}"
          EXISTING_ISSUE=$(gh issue list --label schema-drift --search "${ISSUE_TITLE} in:title" --state open --json number --jq '.[0].number // empty')
          if [ -n "$EXISTING_ISSUE" ]; then
            gh issue close "$EXISTING_ISSUE" --comment "Drift recovered $(date -u +%Y-%m-%d)"
          fi
```

- [ ] **Step 2: Verify YAML syntax**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/nightly-schema.yml'))"
```

Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/nightly-schema.yml
git commit -m "ci: nightly schema-drift workflow with auto-PR + tracking issue"
```

---

## Task 13: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

Add a paragraph documenting the nightly workflows so future contributors and Claude sessions understand the smoketest contract.

- [ ] **Step 1: Read the existing Testing Conventions section**

```bash
grep -n "Testing Conventions" CLAUDE.md
```

- [ ] **Step 2: Append a new section after "Testing Conventions"**

Edit `CLAUDE.md`. Find the line that ends the Testing Conventions section (the last paragraph before the next `##` heading) and add a new section right after it. The new content:

```markdown
## Nightly API Smoketest

Two GitHub Actions workflows run at `0 6 * * *` UTC and on `workflow_dispatch`:

- `.github/workflows/nightly-behavioral.yml` — runs `--ignored` live tests across all four crates' no-auth surfaces. Failures are classified by `.github/scripts/classify_failures.py` into:
  - **auth-gated** (matches the `POLYMARKET_* env vars required` panic) — silently skipped
  - **transient** (HTTP 429/5xx, connection refused, timeouts, DNS) — retried up to 2× with `cargo nextest --retries 2`
  - **real** (everything else) — files or updates a tracking issue with the `nightly-behavioral` label
- `.github/workflows/nightly-schema.yml` — fetches each upstream OpenAPI YAML and compares against `docs/specs/<crate>/openapi.yaml`. On drift, opens an auto-PR (deterministic branch `nightly-schema-drift/<crate>`) and a tracking issue with the `schema-drift` label. The PR commits Polymarket's raw upstream bytes; the canonical-form diff is in the PR body for review.

To enable CLOB/relay's auth-gated tests (currently ~25 + 8 tests), set the `POLYMARKET_*` and `BUILDER_*` repo secrets and remove the `POLYMARKET_\* env vars required` regex from `AUTH_GATED_RE` in `.github/scripts/classify_failures.py`. Auth tests will then start contributing real signal.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude.md): document nightly API smoketest workflows"
```

---

## Task 14: Pre-merge smoke test (manual verification)

This task is not a code change — it's a runtime verification step before opening the PR for merge. It validates that the workflows work end-to-end against real Polymarket APIs.

- [ ] **Step 1: Push the branch**

```bash
git push -u origin aidanb/nightly-api-smoketest
```

- [ ] **Step 2: Trigger nightly-behavioral.yml manually**

```bash
gh workflow run nightly-behavioral.yml --ref aidanb/nightly-api-smoketest
```

Wait ~10 minutes. Then:

```bash
gh run list --workflow nightly-behavioral.yml --branch aidanb/nightly-api-smoketest --limit 1
```

Expected: a `completed/success` run. If failure, inspect logs:

```bash
gh run view --log $(gh run list --workflow nightly-behavioral.yml --branch aidanb/nightly-api-smoketest --limit 1 --json databaseId --jq '.[0].databaseId')
```

- [ ] **Step 3: Verify behavior**

Look for:
- All four matrix entries ran (gamma, data, clob, relay)
- The aggregate job ran
- If any real failures: an issue was created with the `nightly-behavioral` label
- If no real failures: no issue was created

```bash
gh issue list --label nightly-behavioral --state open
```

- [ ] **Step 4: Trigger nightly-schema.yml manually**

```bash
gh workflow run nightly-schema.yml --ref aidanb/nightly-api-smoketest
```

Wait ~5 minutes. Verify completion as above.

- [ ] **Step 5: Verify schema behavior**

Look for:
- Each matrix entry attempted to fetch its upstream YAML
- If any drift: a PR is open on `nightly-schema-drift/<crate>` and an issue with `schema-drift` label is open

```bash
gh pr list --label schema-drift --state open
gh issue list --label schema-drift --state open
```

- [ ] **Step 6: Re-run both workflows to verify dedup**

```bash
gh workflow run nightly-behavioral.yml --ref aidanb/nightly-api-smoketest
gh workflow run nightly-schema.yml --ref aidanb/nightly-api-smoketest
```

Expected behavior:
- If the previous runs created an issue or PR, the second run should **update** it (a comment on the issue, or a force-push to the PR's branch) — not create a new one.
- If the previous runs were clean, the second run should not create anything new.

Verify by counting: `gh issue list --label nightly-behavioral` and `gh pr list --label schema-drift` should not have grown.

- [ ] **Step 7: Document any tweaks**

If the smoke test reveals issues (e.g., real nextest JSON format differs from fixtures, transient regex misses a real-world pattern, gh CLI flag mismatch), fix in code, commit with `fix(ci): ...`, and re-verify with another `workflow_dispatch`.

If everything works, the branch is ready for PR review and merge.

---

## Self-Review Notes

Spec coverage cross-check:

| Spec section | Implementing tasks |
|--------------|---------------------|
| Goal: behavioral drift detection | Tasks 3–5, 11 |
| Goal: schema drift detection | Tasks 7–9, 12 |
| Decision Q1 (no-auth tests) | Task 11 (no `POLYMARKET_*` env in matrix), Task 3/4 (auth-gated classifier) |
| Decision Q2 (issue surfacing) | Task 11 aggregate job |
| Decision Q3 (auto-PR + issue) | Task 12 |
| Decision Q4 (transient-only retry) | Task 3 patterns, Task 11 retry step |
| Decision Q5 (cron + dispatch) | Tasks 11, 12 (`on: schedule + workflow_dispatch`) |
| Component 1: matrix structure | Task 11 |
| Component 1: auth filtering via runtime classification | Tasks 3, 11 |
| Component 1: classify-and-retry flow | Task 5 (CLI), Task 11 (workflow) |
| Component 1: aggregator with reuse-on-repeat | Task 11 aggregate job |
| Component 2: matrix with `relay → relayer` mapping | Task 12 matrix |
| Component 2: per-entry fetch+canonicalize+diff | Tasks 7–9, 12 |
| Component 2: deterministic branch, force-with-lease | Task 12 |
| Component 2: recovery (close stale PR/issue) | Task 12 final step |
| Failure handling: distinct labels | Tasks 11 (`nightly-behavioral`), 12 (`schema-drift`) |
| Failure handling: upstream unreachable | Task 12 fetch step (3 retries, exit 0 on persistent failure) |
| Failure handling: invalid YAML upstream | Task 9 (exit 2), Task 12 (escalate to error) |
| Failure handling: timeout | Tasks 11/12 (`timeout-minutes`) |
| Failure handling: concurrency | Tasks 11/12 (`concurrency:` blocks) |
| Failure handling: permission boundary | Tasks 11/12 (`permissions:` blocks) |
| Testing: unit tests | Tasks 3, 4, 5, 7, 8, 9 |
| Testing: scripts job in ci.yml | Task 10 |
| Testing: workflow-level smoke test | Task 14 |
| Documentation: CLAUDE.md | Task 13 |

No spec section is uncovered.
