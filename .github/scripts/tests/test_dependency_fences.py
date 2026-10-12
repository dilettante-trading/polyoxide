"""Dependencies a member may not take, read from `cargo metadata`.

A venue crate is a member whose `[package.metadata.polyoxide]` names a `venue`.
Its requests go through core's one send loop, so it paces them with the
throttle primitives core exports, never with a rate limiter of its own:

- no venue crate depends directly on `governor`. Core builds its window quotas
  on it; the CLI facade, which is not a venue crate, paces `clob prices
  download` with it.

And each transport has one owner, the one member that declares it, in any
dependency kind, so its features are set in one manifest (`OWNERS`):

- `reqwest` is `polyoxide-core`'s (AD-18, Story 3.13). Every other member
  names its types through `polyoxide_core::reqwest`. (0.37.0's gzip
  regression was a code default in core's builder, not a feature; each HTTP
  module's `tests/headers.rs` is what catches a change of that kind.)
- `tokio-tungstenite` and `rustls` are `polyoxide-ws`'s (Story 4.10). Every
  socket crate connects through the kit, which installs the TLS provider, and
  names tungstenite's types through `polyoxide_ws::tungstenite`.

A copy that arrives through another crate, such as alloy's reqwest 0.13, is
not a direct dependency and passes. Each owner must declare its crate, so the
fence can never pass because nothing declares it.
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
# The one member that declares the socket transport.
SOCKET_OWNER = "polyoxide-ws"
# Each owned crate: its owner, and how every other member reaches it.
OWNERS = {
    "reqwest": (HTTP_OWNER, "every other member names its types through "
                            "`polyoxide_core::reqwest`"),
    "rustls": (SOCKET_OWNER, "every other member connects through the kit, which installs "
                             "the TLS provider"),
    "tokio-tungstenite": (SOCKET_OWNER, "every other member names its types through "
                                        "`polyoxide_ws::tungstenite`"),
}


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


def owner_problems(metadata: dict, owners: dict[str, tuple[str, str]] = OWNERS) -> list[str]:
    """Each member that takes a crate another member owns, in any dependency kind."""
    found = []
    for package in publish_order.members(metadata):
        for dep in package["dependencies"]:
            owned = owners.get(dep["name"])
            if owned is None or package["name"] == owned[0]:
                continue
            owner, reach = owned
            kind = dep["kind"] or "normal"
            found.append(f"{package['name']} takes {dep['name']} as a {kind} dependency; only "
                         f"{owner} declares it, and {reach}")
    return found


def reqwest_problems(metadata: dict) -> list[str]:
    """Each member but core that takes reqwest, in any dependency kind."""
    return owner_problems(metadata, {"reqwest": OWNERS["reqwest"]})


def undeclared(metadata: dict) -> list[str]:
    """Each owned crate its owner does not declare as a normal dependency, which
    would let the fence pass with nothing to fence."""
    members = {p["name"]: p for p in publish_order.members(metadata)}
    return [f"{owner} does not declare {name}" for name, (owner, _) in sorted(OWNERS.items())
            if not any(dep["name"] == name and dep["kind"] is None
                       for dep in members.get(owner, {}).get("dependencies", []))]


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


def test_only_each_owner_depends_on_its_transport() -> None:
    metadata = publish_order.cargo_metadata(REPO / "Cargo.toml")
    assert undeclared(metadata) == [], "the fence would check nothing"
    assert owner_problems(metadata) == []


def test_the_fence_names_a_member_that_depends_on_the_socket_transport() -> None:
    packages = [
        _package("polyoxide-core", None, ["reqwest"]),
        _package("polyoxide-ws", None, ["rustls", "tokio-tungstenite"]),
        _package("polyoxide-clob", "polymarket", ["polyoxide-ws", "rustls"]),
        _package("polyoxide-sports", "polymarket", ["polyoxide-ws"]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert undeclared(metadata) == []
    assert owner_problems(metadata) == [
        "polyoxide-clob takes rustls as a normal dependency; only polyoxide-ws declares it, "
        "and every other member connects through the kit, which installs the TLS provider"
    ]

    # Every kind, and the other owned crate too.
    dev = copy.deepcopy(metadata)
    dev["packages"][3]["dependencies"].append(
        {"name": "tokio-tungstenite", "kind": "dev", "req": "^0.26"})
    dev["packages"][0]["dependencies"].append({"name": "rustls", "kind": "build", "req": "^0.23"})
    assert [p.split(";")[0] for p in owner_problems(dev)] == [
        "polyoxide-core takes rustls as a build dependency",
        "polyoxide-clob takes rustls as a normal dependency",
        "polyoxide-sports takes tokio-tungstenite as a dev dependency",
    ]


def test_an_owner_that_does_not_declare_its_crate_fails_the_fence() -> None:
    packages = [
        _package("polyoxide-core", None, ["reqwest"]),
        _package("polyoxide-ws", None, ["tokio-tungstenite"]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert undeclared(metadata) == ["polyoxide-ws does not declare rustls"]
    # A dev dependency is not the owner's declaration.
    dev = copy.deepcopy(metadata)
    dev["packages"][1]["dependencies"].append({"name": "rustls", "kind": "dev", "req": "^0.23"})
    assert undeclared(dev) == ["polyoxide-ws does not declare rustls"]
