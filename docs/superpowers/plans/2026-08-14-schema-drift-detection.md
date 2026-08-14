# Trustworthy Schema Drift Findings — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every open `schema-drift` issue belong to exactly one spec and state precisely what changed, including drift below path/channel level.

**Architecture:** Extend `.github/scripts/diff_openapi.py` with numeric normalization, a canonical-tree walker producing JSON-pointer changes, and a `render-issue` subcommand that composes a size-budgeted issue body. Rewire `.github/workflows/nightly-schema.yml` to identify issues by a `spec:<id>` label intersection instead of a tokenized title search, and to post bodies with `--body-file`.

**Tech Stack:** Python 3.11+ (stdlib + PyYAML), pytest, `uv`, GitHub Actions, `gh` CLI.

**Spec:** [docs/superpowers/specs/2026-08-14-schema-drift-detection-design.md](../specs/2026-08-14-schema-drift-detection-design.md)

---

## Background an implementer needs

`diff_openapi.py` is run once per spec by a matrix job in `nightly-schema.yml`. It compares Polymarket's published spec against the copy vendored in `docs/specs/`, writes `summary.md` and `unified-diff.txt` into an artifact directory, and exits `0` (no drift), `1` (drift), or `2` (parse error). The workflow reads that exit code and files or closes a GitHub issue.

Two things are broken, both confirmed in run 31778984093:

1. The workflow finds a spec's issue with `gh issue list --search "Schema drift: <id> in:title"`. That is a **tokenized full-text search**, not an exact match, so `perps` also matches `Schema drift: perps-ws`. One job edits and closes another job's issue.
2. `render_summary` enumerates only `paths` and `channels`, so drift inside `components.schemas` produces an issue that names no findings at all.

Run all tests from `.github/scripts`:

```bash
cd .github/scripts && uv run pytest tests/ -v
```

## File structure

| File | Responsibility | Change |
|---|---|---|
| `.github/scripts/diff_openapi.py` | Detection, summary rendering, issue-body composition | Modify |
| `.github/scripts/tests/test_diff_openapi.py` | Unit + CLI tests | Modify |
| `.github/scripts/tests/fixtures/openapi-numeric-noise/` | `3` vs `3.0` pair | Create |
| `.github/scripts/tests/fixtures/openapi-bool-example/` | `'Yes'` vs bare `Yes` pair | Create |
| `.github/workflows/nightly-schema.yml` | Orchestration: labels, lookup, body posting | Modify |
| `CLAUDE.md` | Nightly schema paragraph | Modify |

---

## Task 1: Numeric normalization

YAML parses `3` to Python `int` and `3.0` to `float`, so `json.dumps` renders them differently and `canonicalize` reports drift for a number that did not change. Normalizing integral floats to `int` removes that. The normalizer must check `bool` **first**, because Python's `bool` subclasses `int` — reversing the order would rewrite `True` and erase the genuine `'Yes'` → `Yes` type regression this system exists to catch.

**Files:**
- Create: `.github/scripts/tests/fixtures/openapi-numeric-noise/old.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-numeric-noise/new.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-bool-example/old.yaml`
- Create: `.github/scripts/tests/fixtures/openapi-bool-example/new.yaml`
- Modify: `.github/scripts/diff_openapi.py:28-95`
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Create the numeric-noise fixture pair**

`.github/scripts/tests/fixtures/openapi-numeric-noise/old.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /rewards:
    get:
      summary: Reward rates
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                total_daily_rate: 3
```

`.github/scripts/tests/fixtures/openapi-numeric-noise/new.yaml` — identical but for the last line:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /rewards:
    get:
      summary: Reward rates
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                total_daily_rate: 3.0
```

- [ ] **Step 2: Create the bool-example fixture pair**

`.github/scripts/tests/fixtures/openapi-bool-example/old.yaml`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
components:
  schemas:
    Token:
      type: object
      properties:
        o:
          type: string
          description: Outcome label for the token
          example: 'Yes'
```

`.github/scripts/tests/fixtures/openapi-bool-example/new.yaml` — the quotes are gone, so YAML parses `Yes` as boolean `true`:

```yaml
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
components:
  schemas:
    Token:
      type: object
      properties:
        o:
          type: string
          description: Outcome label for the token
          example: Yes
```

- [ ] **Step 3: Write the failing tests**

Append to `.github/scripts/tests/test_diff_openapi.py`:

```python
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
```

- [ ] **Step 4: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "integral_float or bool_from_string" -v
```

Expected: `test_canonicalize_treats_integral_float_as_int` and `test_detect_drift_ignores_integral_float_noise` FAIL. `test_canonicalize_distinguishes_bool_from_string` already PASSES — it is a regression guard for Step 5, not a driver.

- [ ] **Step 5: Implement `_normalize` and route both readers through it**

In `.github/scripts/diff_openapi.py`, add above `canonicalize`:

```python
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
```

Replace the body of `canonicalize` (currently lines 94-95):

```python
    parsed = _normalize(yaml.safe_load(yaml_text))
    return json.dumps(parsed, sort_keys=True, indent=2)
```

Replace `detect_drift`'s document loading (currently lines 38-39):

```python
    old_doc = _normalized_doc(old_yaml)
    new_doc = _normalized_doc(new_yaml)
```

- [ ] **Step 6: Run the full suite to verify it passes**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS, including all pre-existing tests.

- [ ] **Step 7: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py .github/scripts/tests/fixtures/openapi-numeric-noise .github/scripts/tests/fixtures/openapi-bool-example
git commit -m "fix(ci): treat integral floats as ints when canonicalizing specs

YAML parses 3 and 3.0 to Python int and float, so json.dumps rendered them
differently and canonicalize reported drift for an unchanged number. Booleans
are checked first because bool subclasses int — the reverse order would
rewrite True and erase genuine type drift, which is the other half of the
clob finding."
```

---

## Task 2: Key-path walker

`render_summary` can only describe drift it can name. Today it names paths and channels; drift inside `components.schemas` is invisible. `diff_tree` walks both canonical trees and yields one `Change` per difference, stopping at the root of an added or removed subtree so a new schema is one line rather than forty.

**Files:**
- Modify: `.github/scripts/diff_openapi.py` (add after `_normalized_doc`)
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Write the failing tests**

First extend the imports at the top of `.github/scripts/tests/test_diff_openapi.py`. The current import line is:

```python
from diff_openapi import DriftResult, canonicalize, detect_drift, render_summary
```

Replace it with:

```python
import yaml

from diff_openapi import (
    Change,
    DriftResult,
    canonicalize,
    detect_drift,
    diff_tree,
    render_summary,
)
```

Then append these tests:

```python
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "diff_tree" -v
```

Expected: FAIL at collection with `ImportError: cannot import name 'Change' from 'diff_openapi'`.

- [ ] **Step 3: Implement `Change`, `_plural`, `_render_value`, and `diff_tree`**

In `.github/scripts/diff_openapi.py`, add this block **immediately above the `DriftResult` dataclass** (currently line 17). Task 3 adds a `changes: list[Change]` field to `DriftResult`, so defining `Change` above it keeps the file readable top-to-bottom.

```python
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

    if old != new:
        changes.append(Change(prefix, "changed", _render_value(old), _render_value(new)))
    return changes
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): add canonical-tree walker producing JSON-pointer changes

Drift below path/channel level was undescribable, so clob's issue named no
findings at all. diff_tree reports each difference as a dotted pointer with
before and after values, stopping at the root of an added or removed subtree."
```

---

## Task 3: Surface changes in `DriftResult` and `render_summary`

The walker is useless until the summary prints it. Changes group by top-level key so a reader sees shape (`components` vs `paths`) before detail, and cap at 200 pointers so a large drift stays readable.

**Files:**
- Modify: `.github/scripts/diff_openapi.py` — `DriftResult` (lines 17-25), `detect_drift` (lines 76-84), `render_summary` (lines 98-136)
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Write the failing tests**

Append to `.github/scripts/tests/test_diff_openapi.py`:

```python
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "changes or caps_change" -v
```

Expected: three FAIL, one PASSES.

- `test_render_summary_groups_changes_by_top_level_key` and `test_render_summary_caps_change_enumeration` — `TypeError: DriftResult.__init__() got an unexpected keyword argument 'changes'`
- `test_detect_drift_populates_changes` — `AttributeError: 'DriftResult' object has no attribute 'changes'`
- `test_render_summary_omits_changes_section_when_empty` already passes; it is a guard against emitting an empty `## Changes` heading once the section exists, not a driver.

- [ ] **Step 3: Add the `changes` field to `DriftResult`**

In `.github/scripts/diff_openapi.py`, add one line to the `DriftResult` dataclass, after `channels_modified`:

```python
    changes: list[Change] = field(default_factory=list)
```

Task 2 placed `Change` immediately above `DriftResult`, so this resolves without reordering.

- [ ] **Step 4: Populate it in `detect_drift`**

In the `return DriftResult(...)` at the end of `detect_drift`, add a final argument after `channels_modified=sorted(ch_modified)`:

```python
        changes=diff_tree(old_doc, new_doc),
```

- [ ] **Step 5: Render it in `render_summary`**

Add this helper above `render_summary`:

```python
CHANGE_LIMIT = 200


def _group_of(pointer: str) -> str:
    """Top-level key a pointer belongs to: 'components.schemas.X' -> 'components'."""
    for index, char in enumerate(pointer):
        if char in ".[":
            return pointer[:index]
    return pointer
```

Then, in `render_summary`, insert before the final `return "\n".join(lines)`:

```python
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
```

- [ ] **Step 6: Run the full suite to verify it passes**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): report changed key paths in the drift summary

An issue that says '653 lines differ' and names nothing is not a finding.
Changes group by top-level key and cap at 200 pointers, so both a two-line
clob drift and an 814-line perps-ws drift stay readable."
```

---

## Task 4: `render-issue` subcommand with a size budget

GitHub caps issue and PR bodies at 65536 characters. The workflow currently assembles the body in bash with a fixed `DIFF_LIMIT=50000`, leaving ~15KB of unchecked headroom for the summary — safe only while summaries are four lines, which Task 3 ends. Moving assembly into Python makes the budget a test case.

`--reserve` exists because the workflow appends a `Closes #N` line to the PR variant of this body; reserving those bytes lets one file serve both without exceeding the cap.

**Files:**
- Modify: `.github/scripts/diff_openapi.py` (add `compose_issue_body`, `_cmd_render_issue`, argparse wiring at lines 176-195)
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Write the failing tests**

Extend the import in `.github/scripts/tests/test_diff_openapi.py` to add `compose_issue_body`:

```python
from diff_openapi import (
    Change,
    DriftResult,
    canonicalize,
    compose_issue_body,
    detect_drift,
    diff_tree,
    render_summary,
)
```

Append these tests:

```python
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "issue_body or render_issue" -v
```

Expected: FAIL at collection with `ImportError: cannot import name 'compose_issue_body'`.

- [ ] **Step 3: Implement `compose_issue_body`**

In `.github/scripts/diff_openapi.py`, add after `render_summary`:

```python
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
```

- [ ] **Step 4: Implement the subcommand and wire up argparse**

Add after `_cmd_check`:

```python
def _cmd_render_issue(args: argparse.Namespace) -> int:
    summary = (args.output_dir / "summary.md").read_text()
    diff_path = args.output_dir / "unified-diff.txt"
    diff = diff_path.read_text() if diff_path.exists() else ""
    body = compose_issue_body(summary, diff, reserve=args.reserve)
    (args.output_dir / "issue-body.md").write_text(body)
    return 0
```

In `main`, after the `p.set_defaults(func=_cmd_check)` line:

```python
    p2 = sub.add_parser("render-issue", help="Compose the GitHub issue body from a check's artifacts.")
    p2.add_argument("--output-dir", type=Path, required=True,
                    help="Directory holding summary.md and unified-diff.txt")
    p2.add_argument("--reserve", type=int, default=0,
                    help="Bytes to withhold for text the caller appends (e.g. a PR's `Closes #N`)")
    p2.set_defaults(func=_cmd_render_issue)
```

- [ ] **Step 5: Run the full suite to verify it passes**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): compose the issue body in Python under a tested size budget

Body assembly was bash arithmetic against GitHub's 65536-char cap, with
~15KB of unchecked headroom that held only while summaries were four lines.
Key-path summaries end that, so the budget moves where it can be tested."
```

---

## Task 5: Rewire the workflow

This is the fix for the collision that closed #17 and swallowed perps. Both the open path and the close path must change — the close path is the one that closed another spec's issue.

**Files:**
- Modify: `.github/workflows/nightly-schema.yml:82-104` (Detect drift), `:107-138` (open path), `:140-161` (PR body), `:203-216` (close path)

- [ ] **Step 1: Render the issue body inside the Detect drift step**

This step already runs with `working-directory: .github/scripts`, where `uv` resolves the project. In `.github/workflows/nightly-schema.yml`, replace this block:

```yaml
          if [ "$ec" -eq 0 ]; then
            echo "drift=false" >> "$GITHUB_OUTPUT"
          elif [ "$ec" -eq 1 ]; then
            echo "drift=true" >> "$GITHUB_OUTPUT"
```

with:

```yaml
          if [ "$ec" -eq 0 ]; then
            echo "drift=false" >> "$GITHUB_OUTPUT"
          elif [ "$ec" -eq 1 ]; then
            echo "drift=true" >> "$GITHUB_OUTPUT"
            # Reserve bytes for the `Closes #N` line the PR body appends, so
            # one rendered file is safe as both issue body and PR body.
            uv run python ${{ github.workspace }}/.github/scripts/diff_openapi.py render-issue \
              --output-dir ${{ github.workspace }}/artifacts/${{ matrix.id }} \
              --reserve 64
```

- [ ] **Step 2: Replace label creation and issue lookup in the open path**

Replace lines 127-138 (from the `# --force makes this idempotent` comment through the closing `fi` of the issue create/edit block) with:

```yaml
          # --force makes these idempotent; without the labels existing,
          # `gh issue create --label` / `gh pr create --label` fail outright.
          gh label create schema-drift --force \
            --description "Filed by the nightly schema drift check" --color 1D76DB || true
          gh label create "spec:${SPEC_ID}" --force \
            --description "Schema drift tracking for the ${SPEC_ID} spec" --color 0E8A16 || true

          ISSUE_TITLE="Schema drift: ${SPEC_ID}"
          # Label intersection is exact. `--search "<title> in:title"` is a
          # tokenized full-text search, so `perps` also matched `perps-ws` and
          # one spec's job edited and closed another spec's issue.
          MATCHES=$(gh issue list --label schema-drift --label "spec:${SPEC_ID}" \
            --state open --json number --jq 'sort_by(.number) | map(.number) | join(" ")')
          set -- $MATCHES
          if [ "$#" -gt 1 ]; then
            echo "::warning::${SPEC_ID} has $# open drift issues ($MATCHES); using #$1."
          fi
          EXISTING_ISSUE="${1:-}"
          if [ -z "$EXISTING_ISSUE" ]; then
            EXISTING_ISSUE=$(gh issue create --title "${ISSUE_TITLE}" \
              --label schema-drift --label "spec:${SPEC_ID}" \
              --body-file "${ARTIFACT_DIR}/issue-body.md" | awk -F/ '{print $NF}')
          else
            gh issue edit "$EXISTING_ISSUE" --body-file "${ARTIFACT_DIR}/issue-body.md"
          fi
```

- [ ] **Step 3: Replace the bash PR-body assembly**

Replace lines 140-161 — the block from the `# GitHub caps issue/PR bodies` comment through the closing `} > "$PR_BODY"` — with:

```yaml
          # The rendered body is already under the cap with bytes reserved for
          # the Closes line, so this only appends the linkage.
          PR_BODY=$(mktemp)
          cat "${ARTIFACT_DIR}/issue-body.md" > "$PR_BODY"
          printf '\nCloses #%s\n' "${EXISTING_ISSUE}" >> "$PR_BODY"
```

Then, in the `gh pr create` invocation below it, replace:

```yaml
            --body "$(cat ${PR_BODY})" \
```

with:

```yaml
            --body-file "$PR_BODY" \
```

Also replace the `gh pr edit` line above it:

```yaml
            gh pr edit "$EXISTING_PR" --body "$(cat ${PR_BODY})"
```

with:

```yaml
            gh pr edit "$EXISTING_PR" --body-file "$PR_BODY"
```

- [ ] **Step 4: Fix the close path — this is the step that closed #17**

In the `Close stale drift PR + issue (recovery)` step, replace:

```yaml
          ISSUE_TITLE="Schema drift: ${{ matrix.id }}"
          EXISTING_ISSUE=$(gh issue list --label schema-drift --search "${ISSUE_TITLE} in:title" --state open --json number --jq '.[0].number // empty')
          if [ -n "$EXISTING_ISSUE" ]; then
            gh issue close "$EXISTING_ISSUE" --comment "Drift recovered $(date -u +%Y-%m-%d)"
          fi
```

with:

```yaml
          # Must match the open path exactly. The tokenized title search here
          # is what closed combos-rfq-ws's issue from the combos-rfq job.
          EXISTING_ISSUE=$(gh issue list --label schema-drift --label "spec:${{ matrix.id }}" \
            --state open --json number --jq 'sort_by(.number) | .[0].number // empty')
          if [ -n "$EXISTING_ISSUE" ]; then
            gh issue close "$EXISTING_ISSUE" --comment "Drift recovered $(date -u +%Y-%m-%d)"
          fi
```

- [ ] **Step 5: Verify the workflow still parses**

```bash
python -c "import yaml,sys; yaml.safe_load(open('.github/workflows/nightly-schema.yml')); print('parsed OK')"
```

Expected: `parsed OK`

- [ ] **Step 6: Verify no tokenized title search survives**

```bash
grep -n "in:title" .github/workflows/nightly-schema.yml || echo "no title search remains"
```

Expected: `no title search remains`

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/nightly-schema.yml
git commit -m "fix(ci): identify drift issues by spec label, not tokenized title search

gh issue list --search '<title> in:title' is full-text, not exact: the hyphen
splits tokens, so 'Schema drift: perps' matched the perps-ws issue. In run
31778984093 the perps job wrote its summary into #12 and the combos-rfq job
closed #17, which belongs to combos-rfq-ws. Label intersection is exact, so
the collision class stops existing rather than being handled."
```

---

## Task 6: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` — the `nightly-schema.yml` bullet in the "Nightly API Smoketest" section

- [ ] **Step 1: Replace the drift-reporting sentence**

Find this sentence in the `nightly-schema.yml` bullet:

```markdown
On drift, opens an auto-PR (deterministic branch `nightly-schema-drift/<id>`) and a tracking issue with the `schema-drift` label.
```

Replace it with:

```markdown
On drift, opens an auto-PR (deterministic branch `nightly-schema-drift/<id>`) and a tracking issue labelled `schema-drift` **and** `spec:<id>`. The issue is found by label intersection, never by title: `gh issue list --search "<title> in:title"` is a tokenized full-text search, so `perps` also matches `perps-ws` — that collision let one spec's job edit and close another's issue for eleven days. The issue body carries a key-path summary (changed JSON pointers with before → after) plus the canonicalized diff, composed in Python under GitHub's 65536-character cap.
```

- [ ] **Step 2: Verify the claim about canonicalization is still accurate**

The same section states the PR commits raw upstream bytes while the canonical diff goes in the PR body. Confirm that sentence is unchanged and still true — Task 5 kept `--apply-on-drift` copying raw bytes.

```bash
grep -n "raw upstream bytes" CLAUDE.md
```

Expected: one match, describing the PR contents.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: describe label-based drift issue identity in CLAUDE.md"
```

---

## Task 7: Migration and rollout

Manual and irreversible in part (closing issues), so it runs only after Tasks 1-6 are merged and green. **Confirm with the user before executing this task.**

**Files:** none — GitHub state only.

- [ ] **Step 1: Confirm the full suite and lint are green**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS, no failures.

- [ ] **Step 2: Close the four superseded issues**

Each body regenerates nightly and none carries a comment, so no information is lost.

```bash
for n in 10 12 16 18; do
  gh issue close "$n" --comment "Superseded by the label-based drift tracking in docs/superpowers/specs/2026-08-14-schema-drift-detection-design.md. The next nightly refiles this with a spec: label and a key-path summary."
done
```

- [ ] **Step 3: Trigger the workflow**

```bash
gh workflow run nightly-schema.yml
```

- [ ] **Step 4: Verify the result**

Wait for the run to finish, then:

```bash
gh run list --workflow=nightly-schema.yml --limit 1
gh issue list --label schema-drift --state open --json number,title,labels \
  --jq '.[] | "\(.number) \(.title) [\([.labels[].name] | join(", "))]"'
```

Expected: five open issues — `data`, `bridge`, `clob`, `perps-ws`, and **`perps`**, which has never had one. Each carries exactly two labels: `schema-drift` and its own `spec:<id>`. Each body contains a `## Changes` section.

- [ ] **Step 5: Spot-check the clob issue**

```bash
gh issue list --label "spec:clob" --state open --json number --jq '.[0].number' \
  | xargs -I{} gh issue view {} --json body --jq .body | head -40
```

Expected: a `## Changes` section naming the `example` pointer changing from `"Yes"` to `true`, and **no** mention of `total_daily_rate` — Task 1 normalizes that away. This is the end-to-end proof that both halves of the clob finding behave correctly.

---

## Out of scope

Deferred to S2, per the spec: branch and PR delivery under the no-Actions-PR constraint, a durable won't-sync state so upstream regressions stop refiling, stale-branch cleanup, and severity classification. Task 7 will leave clob refiling nightly — correctly, and now legibly. That is S2's problem to end.
