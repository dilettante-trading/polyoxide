#!/usr/bin/env python3
"""Detect OpenAPI schema drift between Polymarket upstream and our vendored copies."""

from __future__ import annotations

import json
from dataclasses import dataclass, field

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
