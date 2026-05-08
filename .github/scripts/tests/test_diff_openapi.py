"""Unit tests for diff_openapi.py."""

from __future__ import annotations

from pathlib import Path

from diff_openapi import canonicalize, detect_drift

FIXTURES = Path(__file__).parent / "fixtures"


def test_canonicalize_makes_reordered_yaml_equal() -> None:
    old = (FIXTURES / "openapi-no-drift" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-no-drift" / "new.yaml").read_text()
    assert canonicalize(old) == canonicalize(new)


def test_canonicalize_distinguishes_added_endpoint() -> None:
    old = (FIXTURES / "openapi-added-endpoint" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-added-endpoint" / "new.yaml").read_text()
    assert canonicalize(old) != canonicalize(new)


def test_detect_drift_no_drift_returns_clean() -> None:
    old = (FIXTURES / "openapi-no-drift" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-no-drift" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is False
    assert result.endpoints_added == []
    assert result.endpoints_removed == []
    assert result.endpoints_modified == []


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
