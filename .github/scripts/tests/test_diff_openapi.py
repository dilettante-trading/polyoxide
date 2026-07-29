"""Unit tests for diff_openapi.py."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from diff_openapi import DriftResult, canonicalize, detect_drift, render_summary

SCRIPT = Path(__file__).parent.parent / "diff_openapi.py"

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


def test_render_summary_no_drift() -> None:
    text = render_summary(DriftResult(has_drift=False), crate="clob", upstream_url="https://x")
    assert "No drift detected" in text


def test_render_summary_with_changes() -> None:
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


def test_cli_check_parse_error_exits_two(tmp_path: Path) -> None:
    """Invalid YAML upstream should produce exit code 2 with a stderr message."""
    bad_yaml = tmp_path / "bad.yaml"
    bad_yaml.write_text("key: [unclosed\n")
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "test",
            "--upstream-yaml", str(bad_yaml),
            "--vendored-yaml", str(FIXTURES / "openapi-no-drift" / "old.yaml"),
            "--upstream-url", "https://example.com/test.yaml",
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "YAML parse error" in result.stderr
