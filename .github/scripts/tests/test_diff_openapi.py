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


def test_detect_drift_asyncapi_channels_added_and_modified() -> None:
    """AsyncAPI documents key their surface on `channels`, not `paths` —
    without channel awareness a WS-contract drift summary would be empty."""
    old = (FIXTURES / "asyncapi-channel-drift" / "old.json").read_text()
    new = (FIXTURES / "asyncapi-channel-drift" / "new.json").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    assert result.endpoints_added == []
    assert result.channels_added == ["sports"]
    assert result.channels_removed == []
    assert result.channels_modified == ["user"]


def test_detect_drift_asyncapi_channel_removed() -> None:
    old = (FIXTURES / "asyncapi-channel-drift" / "new.json").read_text()
    new = (FIXTURES / "asyncapi-channel-drift" / "old.json").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    assert result.channels_added == []
    assert result.channels_removed == ["sports"]
    assert result.channels_modified == ["user"]


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


def test_render_summary_with_channels() -> None:
    result = DriftResult(
        has_drift=True,
        channels_added=["sports"],
        channels_removed=["legacy"],
        channels_modified=["user"],
    )
    text = render_summary(result, crate="clob-ws-market", upstream_url="https://docs.polymarket.com/asyncapi.json")
    assert "## Channels added" in text
    assert "sports" in text
    assert "## Channels removed" in text
    assert "legacy" in text
    assert "## Channels modified" in text
    assert "user" in text


def test_render_summary_uses_vendored_label() -> None:
    """AsyncAPI mirrors don't live at docs/specs/<crate>/openapi.yaml, so the
    summary prose must name the actual vendored file."""
    result = DriftResult(has_drift=True, channels_added=["sports"])
    text = render_summary(
        result,
        crate="clob-ws-market",
        upstream_url="https://docs.polymarket.com/asyncapi.json",
        vendored_label="docs/specs/clob/asyncapi-market.json",
    )
    assert "docs/specs/clob/asyncapi-market.json" in text
    assert "docs/specs/clob-ws-market/openapi.yaml" not in text


def test_cli_check_asyncapi_drift_with_vendored_label(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "clob-ws-market",
            "--upstream-yaml", str(FIXTURES / "asyncapi-channel-drift" / "new.json"),
            "--vendored-yaml", str(FIXTURES / "asyncapi-channel-drift" / "old.json"),
            "--upstream-url", "https://docs.polymarket.com/asyncapi.json",
            "--vendored-label", "docs/specs/clob/asyncapi-market.json",
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    summary = (out_dir / "summary.md").read_text()
    assert "sports" in summary
    assert "docs/specs/clob/asyncapi-market.json" in summary


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


def test_canonicalize_treats_integral_float_as_int() -> None:
    """YAML `3` and `3.0` are the same JSON number; only Python's int/float
    split makes them differ. Reported drift on clob for exactly this."""
    old = (FIXTURES / "openapi-numeric-noise" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-numeric-noise" / "new.yaml").read_text()
    assert canonicalize(old) == canonicalize(new)


def test_detect_drift_ignores_integral_float_noise() -> None:
    old = (FIXTURES / "openapi-numeric-noise" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-numeric-noise" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is False
    assert result.endpoints_modified == []


def test_canonicalize_distinguishes_bool_from_string() -> None:
    """Unquoted `Yes` parses as boolean True on a `type: string` field — a real
    upstream regression. Python's bool-subclasses-int trap would erase it if
    the normalizer checked isinstance(v, int) before isinstance(v, bool)."""
    old = (FIXTURES / "openapi-bool-example" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-bool-example" / "new.yaml").read_text()
    assert canonicalize(old) != canonicalize(new)
    assert detect_drift(old, new).has_drift is True


def test_canonicalize_distinguishes_bool_from_int() -> None:
    """Mutation guard for _normalize's branch order.

    The 'Yes' fixture cannot detect a reversal: with a string on the old side,
    collapsing True to 1 still leaves the two sides different, so drift is
    still reported and the test stays green. With an int on the old side the
    mutation makes both sides 1 and erases the drift, so this test fails if
    the bool check ever stops preceding the float/int check.
    """
    old = (FIXTURES / "openapi-int-bool-example" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-int-bool-example" / "new.yaml").read_text()
    assert canonicalize(old) != canonicalize(new)
    assert detect_drift(old, new).has_drift is True
