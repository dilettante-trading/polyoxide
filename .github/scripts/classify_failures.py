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
