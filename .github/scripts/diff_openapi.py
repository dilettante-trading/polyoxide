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
