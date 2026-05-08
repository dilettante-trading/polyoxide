#!/usr/bin/env python3
"""Detect OpenAPI schema drift between Polymarket upstream and our vendored copies."""

from __future__ import annotations

import argparse
import difflib
import json
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path

import yaml


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


def canonicalize(yaml_text: str) -> str:
    """Return a canonical string for the YAML's structural content.

    Comments, key ordering, anchors, and YAML-specific syntax are erased:
    we parse to a Python value tree and emit JSON with sorted keys. Two
    YAMLs whose structural content matches will produce identical strings.
    """
    parsed = yaml.safe_load(yaml_text)
    return json.dumps(parsed, sort_keys=True, indent=2)


def render_summary(result: DriftResult, crate: str, upstream_url: str) -> str:
    """Render a markdown summary of a DriftResult."""
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
    """Entry point for the diff_openapi CLI."""
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
