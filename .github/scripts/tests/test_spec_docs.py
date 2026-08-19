"""Consistency checks between vendored API specs and the counts stated in prose.

The nightly schema-drift workflow rewrites `docs/specs/**` with upstream's raw
bytes, but it never touches the prose describing those mirrors. Both perps
counts rotted exactly that way: `docs/specs/perps/INDEX.md` claimed 43
endpoints while the vendored mirror had carried 46 since it was first added.

Nothing else catches this. The drift check compares upstream against the
mirror, not the mirror against our own documentation, so a stale count reads
as authoritative indefinitely. These tests close that gap by re-deriving each
hand-written number from the spec it describes.

Prose is matched whitespace-normalized so reflowing a paragraph cannot break
the assertion; only the number itself is load-bearing.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[3]

# OpenAPI reserves these keys inside a path item for operations; anything else
# (`parameters`, `summary`, `$ref`, `servers`) describes the path as a whole.
HTTP_METHODS = frozenset(
    {"get", "put", "post", "delete", "options", "head", "patch", "trace"}
)


def _document(spec: str) -> dict:
    """Parse a vendored spec. `yaml.safe_load` also accepts JSON."""
    return yaml.safe_load((REPO / spec).read_text()) or {}


def _normalized(doc: str) -> str:
    """Collapse every whitespace run so line wrapping cannot break a match."""
    return " ".join((REPO / doc).read_text().split())


def count_operations(spec: str) -> int:
    """Count HTTP operations across every path in an OpenAPI document."""
    return sum(
        len(HTTP_METHODS.intersection(operations or {}))
        for operations in (_document(spec).get("paths") or {}).values()
    )


def count_channels(spec: str) -> int:
    """Count channels in an AsyncAPI document."""
    return len(_document(spec).get("channels") or {})


# (spec, doc, template) — `template` is the prose claim with the count punched
# out, and must stay specific enough to identify one claim unambiguously.
ENDPOINT_CLAIMS = [
    ("docs/specs/perps/openapi.json", "docs/specs/perps/INDEX.md", "{n} endpoints across four groups."),
    ("docs/specs/perps/openapi.json", "CLAUDE.md", "`perps/`, {n} endpoints on"),
    ("docs/specs/bridge/openapi.yaml", "CLAUDE.md", "(`bridge/`, {n} endpoints)"),
    ("docs/specs/combos-rfq/openapi.yaml", "CLAUDE.md", "(`combos-rfq/`, {n} endpoints)"),
]

CHANNEL_CLAIMS = [
    ("docs/specs/perps/asyncapi.json", "docs/specs/INDEX.md", "Perps WebSocket ({n} channels)"),
]


@pytest.mark.parametrize(
    ("spec", "doc", "template"),
    ENDPOINT_CLAIMS,
    ids=[f"{doc}:{spec}" for spec, doc, template in ENDPOINT_CLAIMS],
)
def test_documented_endpoint_count_matches_spec(spec: str, doc: str, template: str) -> None:
    actual = count_operations(spec)
    claim = template.format(n=actual)
    # Bound to a name first: asserting the `in` directly makes pytest dump the
    # whole normalized document into the failure, burying the message.
    stated = claim in _normalized(doc)
    assert stated, (
        f"{doc} does not state {actual} endpoints for {spec}. "
        f"Expected to find {claim!r}. Refresh the prose to match the mirror."
    )


@pytest.mark.parametrize(
    ("spec", "doc", "template"),
    CHANNEL_CLAIMS,
    ids=[f"{doc}:{spec}" for spec, doc, template in CHANNEL_CLAIMS],
)
def test_documented_channel_count_matches_spec(spec: str, doc: str, template: str) -> None:
    actual = count_channels(spec)
    claim = template.format(n=actual)
    stated = claim in _normalized(doc)
    assert stated, (
        f"{doc} does not state {actual} channels for {spec}. "
        f"Expected to find {claim!r}. Refresh the prose to match the mirror."
    )


@pytest.mark.parametrize(
    ("doc", "template"),
    [(doc, template) for _, doc, template in ENDPOINT_CLAIMS + CHANNEL_CLAIMS],
    ids=[f"{doc}:{template}" for _, doc, template in ENDPOINT_CLAIMS + CHANNEL_CLAIMS],
)
def test_claim_template_still_matches_some_count(doc: str, template: str) -> None:
    """Guard the guard: a reworded claim must fail loudly, not silently pass.

    The count assertions locate prose by substring. Were the surrounding
    wording to change, they would fail with a confusing "count is wrong"
    message when the real problem is that the template no longer describes
    anything in the document. Matching the template against any integer
    separates the two failure modes.
    """
    haystack = _normalized(doc)
    matched = any(template.format(n=n) in haystack for n in range(1000))
    assert matched, (
        f"No claim matching {template!r} found in {doc}. The prose was reworded; "
        f"update the template in this file to match."
    )
