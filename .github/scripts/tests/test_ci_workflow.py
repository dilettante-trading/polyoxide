"""The CI package job checks manifests and can never publish.

It runs on every pull request, so it must hold no registry token, never drop
`--dry-run`, and never be skipped by a condition.
"""

from __future__ import annotations

from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parents[3]
CI = yaml.safe_load((REPO / ".github" / "workflows" / "ci.yml").read_text())
PACKAGE = CI["jobs"]["package"]


def _runs() -> list[str]:
    return [step["run"] for step in PACKAGE["steps"] if "run" in step]


def test_the_package_job_only_dry_runs() -> None:
    publishes = [run for run in _runs() if "cargo publish" in run]
    assert publishes == ["cargo publish --workspace --dry-run --no-verify --locked"]


def test_the_package_job_checks_manifest_metadata() -> None:
    assert "python3 scripts/publish_order.py check-manifests" in _runs()


def test_the_package_job_holds_no_secret() -> None:
    text = yaml.safe_dump(PACKAGE)
    assert "secrets." not in text
    assert "CARGO_REGISTRY_TOKEN" not in text


def test_the_package_job_always_runs() -> None:
    assert "if" not in PACKAGE
    assert "needs" not in PACKAGE
