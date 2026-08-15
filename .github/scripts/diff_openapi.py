#!/usr/bin/env python3
"""Detect OpenAPI schema drift between Polymarket upstream and our vendored copies."""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path

import yaml


@dataclass(frozen=True)
class Change:
    """One difference between two canonical spec trees."""

    pointer: str
    kind: str  # "added" | "removed" | "changed"
    before: str | None
    after: str | None


def _plural(count: int, word: str) -> str:
    return f"{count} {word}" if count == 1 else f"{count} {word}s"


def _render_value(value: object, limit: int = 120) -> str:
    """Render a value for display in a summary line.

    Containers collapse to a shape and a child count: a summary should say a
    schema was added, not reprint it.
    """
    if isinstance(value, dict):
        return f"{{… {_plural(len(value), 'key')}}}"
    if isinstance(value, list):
        return f"[… {_plural(len(value), 'item')}]"
    text = json.dumps(value)
    return text if len(text) <= limit else text[: limit - 1] + "…"


def diff_tree(old: object, new: object, prefix: str = "") -> list[Change]:
    """Walk two canonical trees in parallel, reporting differences as pointers.

    Adds and removes record the subtree root and do not descend, so a newly
    added schema is one Change rather than one per leaf beneath it.
    """
    changes: list[Change] = []

    if isinstance(old, dict) and isinstance(new, dict):
        for key in sorted(set(old) | set(new), key=str):
            child = f"{prefix}.{key}" if prefix else str(key)
            if key not in new:
                changes.append(Change(child, "removed", _render_value(old[key]), None))
            elif key not in old:
                changes.append(Change(child, "added", None, _render_value(new[key])))
            else:
                changes.extend(diff_tree(old[key], new[key], child))
        return changes

    if isinstance(old, list) and isinstance(new, list):
        for index in range(max(len(old), len(new))):
            child = f"{prefix}[{index}]"
            if index >= len(new):
                changes.append(Change(child, "removed", _render_value(old[index]), None))
            elif index >= len(old):
                changes.append(Change(child, "added", None, _render_value(new[index])))
            else:
                changes.extend(diff_tree(old[index], new[index], child))
        return changes

    # `bool` subclasses `int`, so `1 != True` is False and an int->bool change
    # would produce no Change while canonicalize still reports drift — an issue
    # that announces drift and names nothing. Compare type first.
    if type(old) is not type(new) or old != new:
        changes.append(Change(prefix, "changed", _render_value(old), _render_value(new)))
    return changes


@dataclass
class DriftResult:
    has_drift: bool
    endpoints_added: list[str] = field(default_factory=list)
    endpoints_removed: list[str] = field(default_factory=list)
    endpoints_modified: list[str] = field(default_factory=list)
    channels_added: list[str] = field(default_factory=list)
    channels_removed: list[str] = field(default_factory=list)
    channels_modified: list[str] = field(default_factory=list)
    changes: list[Change] = field(default_factory=list)


def detect_drift(old_yaml: str, new_yaml: str) -> DriftResult:
    """Compare two spec strings and report surface-level changes.

    OpenAPI documents key their surface on `paths` (with per-method
    operations); AsyncAPI documents key theirs on `channels`. Both are
    enumerated so the same check serves REST and WebSocket mirrors.
    """
    if canonicalize(old_yaml) == canonicalize(new_yaml):
        return DriftResult(has_drift=False)

    old_doc = _normalized_doc(old_yaml)
    new_doc = _normalized_doc(new_yaml)
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

    old_channels = old_doc.get("channels") or {}
    new_channels = new_doc.get("channels") or {}
    ch_added = [name for name in new_channels if name not in old_channels]
    ch_removed = [name for name in old_channels if name not in new_channels]
    ch_modified = [
        name for name, body in new_channels.items()
        if name in old_channels and old_channels[name] != body
    ]

    return DriftResult(
        has_drift=True,
        endpoints_added=sorted(added),
        endpoints_removed=sorted(removed),
        endpoints_modified=sorted(modified),
        channels_added=sorted(ch_added),
        channels_removed=sorted(ch_removed),
        channels_modified=sorted(ch_modified),
        changes=diff_tree(old_doc, new_doc),
    )


def _normalize(value: object) -> object:
    """Erase YAML's Python-typing artifacts from a parsed value tree.

    `3` and `3.0` parse to int and float and serialize differently, though
    they are the same JSON number. Booleans are returned untouched and are
    checked FIRST: Python's `bool` subclasses `int`, so an isinstance(int)
    test would match True and rewrite it, destroying real type drift.
    """
    if isinstance(value, bool):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    if isinstance(value, dict):
        return {key: _normalize(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_normalize(item) for item in value]
    return value


def _normalized_doc(yaml_text: str) -> dict:
    """Parse a spec into a normalized value tree, defaulting to empty."""
    return _normalize(yaml.safe_load(yaml_text) or {})


def canonicalize(yaml_text: str) -> str:
    """Return a canonical string for the YAML's structural content.

    Comments, key ordering, anchors, and YAML-specific syntax are erased:
    we parse to a Python value tree and emit JSON with sorted keys. Two
    YAMLs whose structural content matches will produce identical strings.
    """
    parsed = _normalize(yaml.safe_load(yaml_text))
    return json.dumps(parsed, sort_keys=True, indent=2)


CHANGE_LIMIT = 200


def _group_of(pointer: str) -> str:
    """Top-level key a pointer belongs to: 'components.schemas.X' -> 'components'."""
    for index, char in enumerate(pointer):
        if char in ".[":
            return pointer[:index]
    return pointer


def render_summary(
    result: DriftResult,
    crate: str,
    upstream_url: str,
    vendored_label: str | None = None,
) -> str:
    """Render a markdown summary of a DriftResult.

    `vendored_label` names the vendored file in prose; it defaults to the
    historical OpenAPI layout for the original four crate entries.
    """
    if vendored_label is None:
        vendored_label = f"docs/specs/{crate}/openapi.yaml"

    if not result.has_drift:
        return f"No drift detected for `{crate}` against `{upstream_url}`.\n"

    lines = [
        f"## Schema drift in `{crate}`",
        "",
        f"Upstream spec at <{upstream_url}> differs from vendored `{vendored_label}`.",
        "",
    ]
    sections = [
        ("## Endpoints added", result.endpoints_added),
        ("## Endpoints removed", result.endpoints_removed),
        ("## Endpoints modified", result.endpoints_modified),
        ("## Channels added", result.channels_added),
        ("## Channels removed", result.channels_removed),
        ("## Channels modified", result.channels_modified),
    ]
    for heading, entries in sections:
        if entries:
            lines.append(heading)
            lines.append("")
            for entry in entries:
                lines.append(f"- `{entry}`")
            lines.append("")

    if result.changes:
        lines.append("## Changes")
        lines.append("")
        shown = result.changes[:CHANGE_LIMIT]
        groups: dict[str, list[Change]] = {}
        for change in shown:
            groups.setdefault(_group_of(change.pointer), []).append(change)
        for group, entries in groups.items():
            lines.append(f"### {group}")
            lines.append("")
            for change in entries:
                if change.kind == "changed":
                    lines.append(f"- `{change.pointer}` changed: `{change.before}` → `{change.after}`")
                elif change.kind == "added":
                    lines.append(f"- `{change.pointer}` added: `{change.after}`")
                else:
                    lines.append(f"- `{change.pointer}` removed: `{change.before}`")
            lines.append("")
        remaining = len(result.changes) - len(shown)
        if remaining:
            lines.append(f"_{remaining} more — see the canonicalized diff below._")
            lines.append("")

    return "\n".join(lines)


ISSUE_BODY_LIMIT = 65536

_DIFF_HEADER = "\n<details><summary>Canonicalized diff</summary>\n\n```diff\n"
_DIFF_FOOTER = "\n```\n</details>\n"
_TRUNCATION_NOTE = "\n… [truncated — full diff in this workflow run's artifacts]"
_OVERFLOW_NOTE = "\n\n_Diff omitted: the summary alone reaches the GitHub body limit. Full detail is in this workflow run's artifacts._\n"


def compose_issue_body(
    summary: str,
    diff: str,
    limit: int = ISSUE_BODY_LIMIT,
    reserve: int = 0,
) -> str:
    """Compose an issue body from a summary and a canonical diff, under `limit`.

    The summary has priority and the diff absorbs any shortfall, because a
    truncated finding is worse than a truncated diff — the full diff is always
    in the run's artifacts. `reserve` withholds bytes for text the caller will
    append afterwards.
    """
    budget = limit - reserve
    if not diff:
        return summary[:budget]

    overhead = len(summary) + len(_DIFF_HEADER) + len(_DIFF_FOOTER) + len(_TRUNCATION_NOTE)
    diff_budget = budget - overhead
    if diff_budget <= 0:
        return summary[: budget - len(_OVERFLOW_NOTE)] + _OVERFLOW_NOTE
    if len(diff) <= diff_budget:
        return summary + _DIFF_HEADER + diff + _DIFF_FOOTER
    return summary + _DIFF_HEADER + diff[:diff_budget] + _TRUNCATION_NOTE + _DIFF_FOOTER


def diff_fingerprint(diff_text: str) -> str:
    """SHA-256 hex digest of a canonical unified diff.

    Fingerprints the *disagreement* rather than the upstream document, so an
    acknowledgement expires in both directions: it stops matching if upstream
    changes, and also if we adopt part of the drift. Hashing upstream alone
    would stay silent after a partial adoption, exactly when a fresh look is
    warranted.
    """
    return hashlib.sha256(diff_text.encode("utf-8")).hexdigest()


def load_acknowledged(path: Path | None) -> dict:
    """Read the acknowledgement file, tolerating absence and corruption.

    Every failure resolves to 'nothing is acknowledged'. Silence must only
    ever be produced by an exact hash match, never by a missing or unreadable
    file — a config mistake has to make the workflow louder, not quieter.
    """
    if path is None or not path.exists():
        return {}
    try:
        parsed = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError) as exc:
        print(f"::warning::could not read {path}: {exc}", file=sys.stderr)
        return {}
    if not isinstance(parsed, dict):
        print(f"::warning::{path} is not a JSON object; ignoring", file=sys.stderr)
        return {}
    return parsed


def _cmd_check(args: argparse.Namespace) -> int:
    upstream_yaml = args.upstream_yaml.read_text()
    vendored_yaml = args.vendored_yaml.read_text()
    try:
        result = detect_drift(vendored_yaml, upstream_yaml)
    except yaml.YAMLError as exc:
        print(f"YAML parse error: {exc}", file=sys.stderr)
        return 2

    args.output_dir.mkdir(parents=True, exist_ok=True)
    vendored_label = args.vendored_label or f"docs/specs/{args.crate}/openapi.yaml"
    summary = render_summary(
        result,
        crate=args.crate,
        upstream_url=args.upstream_url,
        vendored_label=vendored_label,
    )
    (args.output_dir / "summary.md").write_text(summary)

    if not result.has_drift:
        return 0

    canonical_old = canonicalize(vendored_yaml).splitlines(keepends=True)
    canonical_new = canonicalize(upstream_yaml).splitlines(keepends=True)
    diff = "".join(difflib.unified_diff(
        canonical_old, canonical_new,
        fromfile=f"vendored {vendored_label} (canonical)",
        tofile=f"upstream {args.upstream_url} (canonical)",
    ))
    (args.output_dir / "unified-diff.txt").write_text(diff)

    fingerprint = diff_fingerprint(diff)
    (args.output_dir / "diff-sha256.txt").write_text(fingerprint + "\n")

    entry = load_acknowledged(args.acknowledged_file).get(args.crate) or {}
    if entry.get("diff_sha256") == fingerprint:
        # Acknowledged: we decided not to sync, so the mirror must not be
        # overwritten even if --apply-on-drift was passed.
        return 3

    if args.apply_on_drift:
        shutil.copyfile(args.upstream_yaml, args.vendored_yaml)

    return 1


def _cmd_render_issue(args: argparse.Namespace) -> int:
    summary = (args.output_dir / "summary.md").read_text()
    diff_path = args.output_dir / "unified-diff.txt"
    diff = diff_path.read_text() if diff_path.exists() else ""
    body = compose_issue_body(summary, diff, reserve=args.reserve)
    (args.output_dir / "issue-body.md").write_text(body)
    return 0


def main(argv: list[str] | None = None) -> int:
    """Entry point for the diff_openapi CLI."""
    parser = argparse.ArgumentParser(description="Detect OpenAPI schema drift.")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("check", help="Compare upstream vs vendored OpenAPI.")
    p.add_argument("--crate", required=True, help="e.g. clob, gamma, data, relay")
    p.add_argument("--upstream-yaml", type=Path, required=True, help="Path to fetched upstream YAML")
    p.add_argument("--vendored-yaml", type=Path, required=True, help="Path to docs/specs/<crate>/openapi.yaml")
    p.add_argument("--upstream-url", required=True, help="URL the upstream was fetched from (for the summary)")
    p.add_argument("--vendored-label", default=None,
                   help="Repo-relative path of the vendored file, for summary prose "
                        "(defaults to docs/specs/<crate>/openapi.yaml)")
    p.add_argument("--output-dir", type=Path, required=True)
    p.add_argument("--apply-on-drift", action="store_true",
                   help="If drift detected, overwrite vendored-yaml with upstream-yaml's raw bytes")
    p.add_argument("--acknowledged-file", type=Path, default=None,
                   help="JSON file of accepted-as-different specs, keyed by crate id "
                        "with a diff_sha256 field; a match exits 3 instead of 1")
    p.set_defaults(func=_cmd_check)

    p2 = sub.add_parser("render-issue", help="Compose the GitHub issue body from a check's artifacts.")
    p2.add_argument("--output-dir", type=Path, required=True,
                    help="Directory holding summary.md and unified-diff.txt")
    p2.add_argument("--reserve", type=int, default=0,
                    help="Bytes to withhold for text the caller appends (e.g. a PR's `Closes #N`)")
    p2.set_defaults(func=_cmd_render_issue)

    ns = parser.parse_args(argv)
    return ns.func(ns)


if __name__ == "__main__":
    sys.exit(main())
