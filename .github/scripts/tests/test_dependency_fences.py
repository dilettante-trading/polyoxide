"""Dependencies a venue crate may not take, read from `cargo metadata`.

A venue crate is a member whose `[package.metadata.polyoxide]` names a `venue`.
Its requests go through core's one send loop, so it paces them with the
throttle primitives core exports, never with a rate limiter of its own:

- no venue crate depends directly on `governor`. Core builds its window quotas
  on it; the CLI facade, which is not a venue crate, paces `clob prices
  download` with it.

Story 3.13 extends this file to `reqwest`.
"""

from __future__ import annotations

import copy
import importlib.util
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
FENCED = frozenset({"governor"})


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


def venue_crates(metadata: dict) -> list[dict]:
    """The members that declare a venue."""
    return [p for p in publish_order.members(metadata)
            if ((p.get("metadata") or {}).get("polyoxide") or {}).get("venue")]


def problems(metadata: dict) -> list[str]:
    """Each fenced dependency a venue crate takes, of any kind."""
    found = []
    for package in venue_crates(metadata):
        for dep in package["dependencies"]:
            if dep["name"] in FENCED:
                kind = dep["kind"] or "normal"
                found.append(f"{package['name']} takes {dep['name']} as a {kind} dependency; a "
                             f"venue crate paces requests with core's throttle primitives")
    return found


def test_no_venue_crate_depends_on_governor() -> None:
    metadata = publish_order.cargo_metadata(REPO / "Cargo.toml")
    assert venue_crates(metadata), "no member declares a venue, so the fence checks nothing"
    assert problems(metadata) == []


def _package(name: str, venue: str | None, dependencies: list[str]) -> dict:
    """A member as `cargo metadata --no-deps` reports it, cut to what the fence reads."""
    return {
        "id": f"path+file:///repo/{name}#0.38.1",
        "name": name,
        "metadata": {"polyoxide": {"venue": venue}} if venue else None,
        "dependencies": [{"name": dep, "kind": None, "req": "^0.3"} for dep in dependencies],
    }


def test_the_fence_names_a_venue_crate_that_depends_on_governor() -> None:
    packages = [
        _package("polyoxide-core", None, ["governor", "reqwest"]),
        _package("polyoxide-cli", None, ["governor"]),
        _package("polyoxide-gamma", "polymarket", ["polyoxide-core"]),
        _package("polyoxide-binance", "binance", ["polyoxide-core", "governor"]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert problems(metadata) == [
        "polyoxide-binance takes governor as a normal dependency; a venue crate paces "
        "requests with core's throttle primitives"
    ]

    dev = copy.deepcopy(metadata)
    dev["packages"][2]["dependencies"].append({"name": "governor", "kind": "dev", "req": "^0.6"})
    assert [p.split(";")[0] for p in problems(dev)] == [
        "polyoxide-gamma takes governor as a dev dependency",
        "polyoxide-binance takes governor as a normal dependency",
    ]
