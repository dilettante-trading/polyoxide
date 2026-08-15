"""Unit tests for diff_openapi.py."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest
import yaml

from diff_openapi import (
    Change,
    DriftResult,
    canonicalize,
    compose_issue_body,
    detect_drift,
    diff_fingerprint,
    diff_tree,
    render_summary,
)

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


def test_diff_tree_reports_added_schema_property() -> None:
    """The polyoxide-data case: a new property on an existing schema should be
    named, not reported as an opaque 'endpoint modified'."""
    old = yaml.safe_load((FIXTURES / "openapi-modified-schema" / "old.yaml").read_text())
    new = yaml.safe_load((FIXTURES / "openapi-modified-schema" / "new.yaml").read_text())
    changes = diff_tree(old, new)
    assert len(changes) == 1
    (change,) = changes
    assert change.pointer.endswith("properties.creator_address")
    assert change.kind == "added"
    assert change.before is None
    assert change.after == "{… 1 key}"


def test_diff_tree_reports_subtree_root_not_leaves() -> None:
    """An added subtree yields one Change at its root. Descending would turn a
    new schema into dozens of pointers and bury the finding."""
    old = {"components": {"schemas": {}}}
    new = {"components": {"schemas": {"Approval": {"type": "object", "x": 1, "y": 2}}}}
    changes = diff_tree(old, new)
    assert len(changes) == 1
    assert changes[0] == Change(
        pointer="components.schemas.Approval",
        kind="added",
        before=None,
        after="{… 3 keys}",
    )


def test_diff_tree_reports_changed_scalar_with_both_values() -> None:
    old = {"components": {"schemas": {"Token": {"properties": {"o": {"example": "Yes"}}}}}}
    new = {"components": {"schemas": {"Token": {"properties": {"o": {"example": True}}}}}}
    changes = diff_tree(old, new)
    assert changes == [
        Change(
            pointer="components.schemas.Token.properties.o.example",
            kind="changed",
            before='"Yes"',
            after="true",
        )
    ]


def test_diff_tree_reports_removed_key() -> None:
    changes = diff_tree({"info": {"version": "1.0.0"}}, {"info": {}})
    assert changes == [
        Change(pointer="info.version", kind="removed", before='"1.0.0"', after=None)
    ]


def test_diff_tree_indexes_list_elements() -> None:
    changes = diff_tree({"servers": ["a", "b"]}, {"servers": ["a", "c"]})
    assert changes == [
        Change(pointer="servers[1]", kind="changed", before='"b"', after='"c"')
    ]


def test_diff_tree_truncates_long_scalar_values() -> None:
    changes = diff_tree({"d": "x" * 400}, {"d": "y" * 400})
    assert len(changes[0].after) == 120
    assert changes[0].after.endswith("…")


def test_detect_drift_populates_changes() -> None:
    old = (FIXTURES / "openapi-modified-schema" / "old.yaml").read_text()
    new = (FIXTURES / "openapi-modified-schema" / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    assert len(result.changes) == 1
    assert result.changes[0].pointer.endswith("properties.creator_address")


def test_render_summary_groups_changes_by_top_level_key() -> None:
    result = DriftResult(
        has_drift=True,
        changes=[
            Change("components.schemas.Position.properties.entryFeesUsdc", "added", None, "{… 2 keys}"),
            Change("info.version", "changed", '"1.0.0"', '"1.1.0"'),
        ],
    )
    text = render_summary(result, crate="data", upstream_url="https://x")
    assert "## Changes" in text
    assert "### components" in text
    assert "### info" in text
    assert "`components.schemas.Position.properties.entryFeesUsdc` added: `{… 2 keys}`" in text
    assert "`info.version` changed: `\"1.0.0\"` → `\"1.1.0\"`" in text


def test_render_summary_caps_change_enumeration() -> None:
    result = DriftResult(
        has_drift=True,
        changes=[Change(f"paths./p{i}", "added", None, "{… 1 key}") for i in range(250)],
    )
    text = render_summary(result, crate="perps-ws", upstream_url="https://x")
    assert "`paths./p199` added" in text
    assert "`paths./p200` added" not in text
    assert "50 more" in text


def test_render_summary_omits_changes_section_when_empty() -> None:
    result = DriftResult(has_drift=True, endpoints_added=["POST /new"])
    text = render_summary(result, crate="clob", upstream_url="https://x")
    assert "## Changes" not in text


def test_diff_tree_distinguishes_int_from_bool() -> None:
    """Same bool-subclasses-int trap as _normalize, one layer up: plain `!=`
    finds 1 and True equal, so an int->bool change produced no Change while
    has_drift was still True."""
    changes = diff_tree({"example": 1}, {"example": True})
    assert changes == [
        Change(pointer="example", kind="changed", before="1", after="true")
    ]


@pytest.mark.parametrize(
    "fixture",
    [
        "openapi-added-endpoint",
        "openapi-removed-endpoint",
        "openapi-modified-schema",
        "openapi-bool-example",
        "openapi-int-bool-example",
    ],
)
def test_drifting_fixture_always_names_a_finding(fixture: str) -> None:
    """A summary that reports drift but names nothing is the exact failure this
    change exists to eliminate — the clob issue did that for weeks. Any fixture
    with drift must yield at least one named finding.
    """
    old = (FIXTURES / fixture / "old.yaml").read_text()
    new = (FIXTURES / fixture / "new.yaml").read_text()
    result = detect_drift(old, new)
    assert result.has_drift is True
    named = (
        result.endpoints_added
        + result.endpoints_removed
        + result.endpoints_modified
        + result.channels_added
        + result.channels_removed
        + result.channels_modified
        + [change.pointer for change in result.changes]
    )
    assert named, f"{fixture} reports drift but names no finding"


def test_compose_issue_body_embeds_diff_when_it_fits() -> None:
    body = compose_issue_body("## Schema drift in `clob`\n", "-old\n+new\n")
    assert "## Schema drift in `clob`" in body
    assert "<details><summary>Canonicalized diff</summary>" in body
    assert "-old" in body
    assert "truncated" not in body


def test_compose_issue_body_within_cap() -> None:
    """Summary has priority; the diff yields. Both oversized must still fit."""
    summary = "S" * 30_000
    diff = "D" * 90_000
    body = compose_issue_body(summary, diff)
    assert len(body) <= 65536
    assert body.startswith("S" * 30_000)
    assert "truncated" in body
    assert body.rstrip().endswith("</details>")


def test_compose_issue_body_drops_diff_when_summary_exceeds_budget() -> None:
    body = compose_issue_body("S" * 70_000, "D" * 1_000)
    assert len(body) <= 65536
    assert "<details>" not in body
    assert "run's artifacts" in body


def test_compose_issue_body_without_diff_returns_summary() -> None:
    body = compose_issue_body("## No drift\n", "")
    assert body == "## No drift\n"


def test_compose_issue_body_reserve_shrinks_the_budget() -> None:
    """The PR variant appends `Closes #N`; reserved bytes keep it under cap."""
    body = compose_issue_body("S" * 100, "D" * 90_000, reserve=1_000)
    assert len(body) <= 65536 - 1_000


def test_cli_render_issue_writes_body(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
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
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "render-issue", "--output-dir", str(out_dir)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    body = (out_dir / "issue-body.md").read_text()
    assert "GET /markets/{id}" in body
    assert "<details><summary>Canonicalized diff</summary>" in body
    assert len(body) <= 65536


def test_diff_fingerprint_is_stable() -> None:
    """Same diff text must hash identically across calls, or an
    acknowledgement would expire at random."""
    text = "-old line\n+new line\n"
    assert diff_fingerprint(text) == diff_fingerprint(text)
    assert len(diff_fingerprint(text)) == 64


def test_diff_fingerprint_changes_with_diff() -> None:
    """A one-character change must break the match, so acknowledging one
    disagreement never silences a different one."""
    assert diff_fingerprint("-a\n+b\n") != diff_fingerprint("-a\n+c\n")


def test_diff_fingerprint_of_empty_string() -> None:
    assert len(diff_fingerprint("")) == 64
