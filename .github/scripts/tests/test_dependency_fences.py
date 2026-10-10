"""Dependencies a member may not take, read from `cargo metadata`.

A venue crate is a member whose `[package.metadata.polyoxide]` names a `venue`.
Its requests go through core's one send loop, so it paces them with the
throttle primitives core exports, never with a rate limiter of its own:

- no venue crate depends directly on `governor`. Core builds its window quotas
  on it; the CLI facade, which is not a venue crate, paces `clob prices
  download` with it.

And core owns the HTTP client (AD-18, Story 3.13):

- no member but `polyoxide-core` depends directly on `reqwest`, in any
  dependency kind. Every other member names its types through
  `polyoxide_core::reqwest`, so no crate can change reqwest's features for the
  rest, which is how 0.37.0 took gzip away from every client. A copy that
  arrives through another crate, such as alloy's reqwest 0.13, is not a direct
  dependency and passes.
"""

from __future__ import annotations

import copy
import importlib.util
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
FENCED = frozenset({"governor"})
# The one member that declares reqwest.
HTTP_OWNER = "polyoxide-core"


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


def reqwest_problems(metadata: dict) -> list[str]:
    """Each member but core that takes reqwest, in any dependency kind."""
    found = []
    for package in publish_order.members(metadata):
        if package["name"] == HTTP_OWNER:
            continue
        for dep in package["dependencies"]:
            if dep["name"] == "reqwest":
                kind = dep["kind"] or "normal"
                found.append(f"{package['name']} takes reqwest as a {kind} dependency; only "
                             f"{HTTP_OWNER} declares it, and every other member names its "
                             f"types through `polyoxide_core::reqwest`")
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


def test_only_core_depends_on_reqwest() -> None:
    metadata = publish_order.cargo_metadata(REPO / "Cargo.toml")
    [core] = [p for p in publish_order.members(metadata) if p["name"] == HTTP_OWNER]
    assert any(dep["name"] == "reqwest" and dep["kind"] is None for dep in core["dependencies"]), (
        f"{HTTP_OWNER} does not declare reqwest, so the fence checks nothing")
    assert reqwest_problems(metadata) == []


def test_the_fence_names_a_member_that_depends_on_reqwest() -> None:
    packages = [
        _package("polyoxide-core", None, ["governor", "reqwest"]),
        _package("polyoxide-cli", None, ["polyoxide-core"]),
        _package("polyoxide-gamma", "polymarket", ["polyoxide-core", "reqwest"]),
        _package("polyoxide-test-support", None, ["polyoxide-core"]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert reqwest_problems(metadata) == [
        "polyoxide-gamma takes reqwest as a normal dependency; only polyoxide-core declares "
        "it, and every other member names its types through `polyoxide_core::reqwest`"
    ]

    # Every kind, and members that are not venue crates too.
    dev = copy.deepcopy(metadata)
    dev["packages"][3]["dependencies"].append({"name": "reqwest", "kind": "dev", "req": "^0.12"})
    dev["packages"][1]["dependencies"].append({"name": "reqwest", "kind": "build", "req": "^0.12"})
    assert [p.split(";")[0] for p in reqwest_problems(dev)] == [
        "polyoxide-cli takes reqwest as a build dependency",
        "polyoxide-gamma takes reqwest as a normal dependency",
        "polyoxide-test-support takes reqwest as a dev dependency",
    ]

    # Core's own declaration is the one that is allowed.
    alone = copy.deepcopy(metadata)
    alone["packages"][2]["dependencies"].remove(
        {"name": "reqwest", "kind": None, "req": "^0.3"})
    assert reqwest_problems(alone) == []
