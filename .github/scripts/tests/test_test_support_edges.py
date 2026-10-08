"""polyoxide-test-support's dependency edges, read from `cargo metadata`.

The toolkit every live suite shares must stay out of every build but a test's:

- it depends on `polyoxide-core` and `polyoxide-venue` alone, never on a venue
  crate, so a suite builds its own client and the toolkit names no venue;
- every crate takes it as a path-only dev-dependency, which cargo strips when it
  publishes that crate, since the toolkit is never published itself;
- core and venue never take it at all: it depends on them.
"""

from __future__ import annotations

import copy
import importlib.util
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
TEST_SUPPORT = "polyoxide-test-support"
ALLOWED = frozenset({"polyoxide-core", "polyoxide-venue"})


def _load_publish_order():
    """`scripts/publish_order.py`, which lives outside this uv project."""
    if "publish_order" in sys.modules:
        return sys.modules["publish_order"]
    spec = importlib.util.spec_from_file_location("publish_order", REPO / "scripts" / "publish_order.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


publish_order = _load_publish_order()
METADATA = publish_order.workspace_metadata()


def problems(metadata: dict) -> list[str]:
    """Every edge into or out of the toolkit that breaks the rules above."""
    members = {p["name"]: p for p in publish_order.members(metadata)}
    support = members.get(TEST_SUPPORT)
    if support is None:
        return [f"{TEST_SUPPORT} is not a workspace member"]
    found = []
    if publish_order.is_publishable(support):
        found.append(f"{TEST_SUPPORT} must be `publish = false`")
    for dep in support["dependencies"]:
        if dep.get("path") is not None and dep["name"] in members and dep["name"] not in ALLOWED:
            found.append(f"{TEST_SUPPORT} depends on {dep['name']}; of the workspace it may "
                         f"depend on {', '.join(sorted(ALLOWED))} only")
    for name, package in members.items():
        for dep in package["dependencies"]:
            if dep["name"] != TEST_SUPPORT:
                continue
            if name in ALLOWED:
                found.append(f"{name} depends on {TEST_SUPPORT}, which depends on {name}")
            elif dep["kind"] != "dev":
                found.append(f"{name} takes {TEST_SUPPORT} as a {dep['kind'] or 'normal'} "
                             f"dependency; take it under [dev-dependencies]")
            elif not publish_order.is_path_only(dep):
                found.append(f"{name}'s dev-dependency on {TEST_SUPPORT} carries a version or "
                             f"no path; give it a `path` alone, so publishing {name} strips it")
    return found


def test_the_real_workspace_keeps_every_rule() -> None:
    assert problems(METADATA) == []


def test_the_toolkit_depends_on_core_and_venue() -> None:
    """Guards the subset check above against a toolkit that depends on nothing."""
    support = next(p for p in publish_order.members(METADATA) if p["name"] == TEST_SUPPORT)
    workspace = {p["name"] for p in publish_order.members(METADATA)}
    assert {d["name"] for d in support["dependencies"] if d["name"] in workspace} == ALLOWED


# --- each rule, broken --------------------------------------------------------


def _edge(name: str, kind: str | None = "dev", req: str = "*", path: bool = True) -> dict:
    """A dependency as `cargo metadata` reports it: a path-only one reads back as `*`."""
    return {"name": name, "kind": kind, "req": req,
            "path": str(REPO / name) if path else None, "optional": False,
            "uses_default_features": True, "features": []}


def _with(edges: dict[str, list[dict]]) -> dict:
    """The real metadata, with `edges` added to the named members."""
    metadata = copy.deepcopy(METADATA)
    for package in metadata["packages"]:
        package["dependencies"] += edges.get(package["name"], [])
    return metadata


def test_a_path_only_dev_dependency_is_allowed() -> None:
    assert problems(_with({"polyoxide-gamma": [_edge(TEST_SUPPORT)]})) == []


@pytest.mark.parametrize("edges,fault", [
    ({TEST_SUPPORT: [_edge("polyoxide-clob", kind=None, req="^0.38.1")]},
     "polyoxide-test-support depends on polyoxide-clob"),
    ({TEST_SUPPORT: [_edge("polyoxide-gamma")]},
     "polyoxide-test-support depends on polyoxide-gamma"),
    ({"polyoxide-gamma": [_edge(TEST_SUPPORT, kind=None)]},
     "polyoxide-gamma takes polyoxide-test-support as a normal dependency"),
    ({"polyoxide-gamma": [_edge(TEST_SUPPORT, kind="build")]},
     "polyoxide-gamma takes polyoxide-test-support as a build dependency"),
    ({"polyoxide-gamma": [_edge(TEST_SUPPORT, req="^0.38.1")]},
     "polyoxide-gamma's dev-dependency on polyoxide-test-support carries a version"),
    ({"polyoxide-gamma": [_edge(TEST_SUPPORT, path=False)]},
     "polyoxide-gamma's dev-dependency on polyoxide-test-support carries a version or no path"),
    ({"polyoxide-core": [_edge(TEST_SUPPORT)]},
     "polyoxide-core depends on polyoxide-test-support"),
    ({"polyoxide-venue": [_edge(TEST_SUPPORT)]},
     "polyoxide-venue depends on polyoxide-test-support"),
], ids=["venue-crate", "venue-crate-dev", "normal", "build", "versioned", "registry", "core", "venue"])
def test_each_broken_rule_is_caught(edges: dict, fault: str) -> None:
    found = problems(_with(edges))
    assert len(found) == 1 and found[0].startswith(fault), found


def test_a_published_toolkit_is_caught() -> None:
    metadata = copy.deepcopy(METADATA)
    support = next(p for p in metadata["packages"] if p["name"] == TEST_SUPPORT)
    support["publish"] = None
    assert problems(metadata) == [f"{TEST_SUPPORT} must be `publish = false`"]
