#!/usr/bin/env python3
"""Classify cargo nextest failures into auth-gated / transient / real."""

from __future__ import annotations

import json
import re
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
