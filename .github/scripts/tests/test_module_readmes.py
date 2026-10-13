"""Each module of a venue crate has a README that is compiled as a doctest.

A member that declares a `venue` and its `products` holds a module per product in
`src/<product>/`, and that module's `src/<product>/README.md` is compiled under the
module's own feature by one include in the crate's sources, as polyoxide-polymarket's
lib.rs does:

    #[cfg(all(doctest, feature = "gamma"))]
    #[doc = include_str!("gamma/README.md")]
    struct GammaReadmeDoctests;

A README no include names is never compiled, so its examples rot unseen; an include
whose README is not there fails only `cargo test --doc` with that feature on. These
tests fail on both, naming the module, so the module READMEs and their gated includes
are counted one for one. A module directory without its README fails too. Only a product
its crate builds behind a feature of the product's name is a module: a crate that is still
its one module (perps, clob), or one whose module has no feature yet (Binance's `usdm`
until Story 5.6), is not checked.
"""

from __future__ import annotations

import importlib.util
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
# A doctest include under one feature: the gate, then the include it gates.
INCLUDE = re.compile(
    r'#\[cfg\(all\(doctest,\s*feature\s*=\s*"(?P<feature>[^"]+)"\)\)\]\s*'
    r'#\[doc\s*=\s*include_str!\("(?P<path>[^"]+)"\)\]')


def _load_publish_order():
    """`scripts/publish_order.py`, which lives outside this uv project."""
    if "publish_order" in sys.modules:
        return sys.modules["publish_order"]
    spec = importlib.util.spec_from_file_location("publish_order", REPO / "scripts" / "publish_order.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@dataclass(frozen=True)
class Include:
    feature: str  # the feature its `cfg(all(doctest, feature = ...))` names
    path: str  # what its `include_str!` names
    target: Path  # that, resolved against the including file's directory
    source: Path  # the including file


def includes(src: Path) -> list[Include]:
    """Every doctest include under `src` gated on one feature."""
    found = []
    for path in sorted(src.rglob("*.rs")):
        for match in INCLUDE.finditer(path.read_text(encoding="utf-8")):
            target = (path.parent / match["path"]).resolve()
            found.append(Include(match["feature"], match["path"], target, path))
    return found


def problems(crate: Path, products: list[str]) -> list[str]:
    """`<crate>: module `<m>` ...` for each module directory without its README, each
    module README that is not included exactly once under its own feature, and each
    include of a product's feature whose README is not a file."""
    src = crate / "src"
    found = includes(src)
    out = []
    for product in products:
        readme = (src / product / "README.md").resolve()
        if not (src / product).is_dir():
            continue
        if not readme.is_file():
            out.append(f"{crate.name}: module `{product}` has no README.md in "
                       f"src/{product}/")
            continue
        gated = [i for i in found if i.target == readme and i.feature == product]
        if len(gated) != 1:
            out.append(f"{crate.name}: module `{product}`'s README.md is included "
                       f"{len(gated)} times under `cfg(all(doctest, feature = \"{product}\"))`, "
                       f"not once")
    for include in found:
        if include.feature in products and not include.target.is_file():
            out.append(f"{crate.name}: module `{include.feature}`'s include in "
                       f"{include.source.relative_to(crate).as_posix()} names "
                       f"{include.path}, which is not a file")
    return out


def venue_modules(metadata: dict) -> list[tuple[Path, list[str]]]:
    """Each member that declares a venue and products, with the products it builds
    behind a feature of the product's name: its modules."""
    out = []
    for package in _load_publish_order().members(metadata):
        meta = (package.get("metadata") or {}).get("polyoxide") or {}
        features = package.get("features") or {}
        modules = [p for p in meta.get("products") or () if p in features]
        if meta.get("venue") and modules:
            out.append((Path(package["manifest_path"]).parent, modules))
    return out


# --- the real tree -----------------------------------------------------------


def test_every_module_readme_is_a_doctest_under_its_feature() -> None:
    metadata = _load_publish_order().workspace_metadata()
    crates = venue_modules(metadata)
    found = [p for crate, products in crates for p in problems(crate, products)]
    assert not found, "\n".join(found)
    readmes = [(crate.name, product) for crate, products in crates for product in products
               if (crate / "src" / product / "README.md").is_file()]
    assert len(readmes) >= 2, f"too few module READMEs found, so the check is vacuous: {readmes}"


# --- synthetic crates --------------------------------------------------------


GAMMA = '''\
#[cfg(all(doctest, feature = "gamma"))]
#[doc = include_str!("gamma/README.md")]
struct GammaReadmeDoctests;
'''

DATA = '''\
#[cfg(all(doctest, feature = "data"))]
#[doc = include_str!("data/README.md")]
struct DataReadmeDoctests;
'''


def _crate(root: Path, lib: str, readmes: list[str]) -> Path:
    crate = root / "polyoxide-venue-crate"
    (crate / "src").mkdir(parents=True)
    (crate / "src" / "lib.rs").write_text(lib)
    for module in readmes:
        (crate / "src" / module).mkdir()
        (crate / "src" / module / "README.md").write_text(f"# {module}\n")
    return crate


def test_each_readme_with_its_include_passes(tmp_path: Path) -> None:
    crate = _crate(tmp_path, GAMMA + DATA, ["gamma", "data"])
    assert problems(crate, ["gamma", "data"]) == []


def test_a_readme_without_an_include_is_caught(tmp_path: Path) -> None:
    crate = _crate(tmp_path, GAMMA, ["gamma", "data"])
    assert problems(crate, ["gamma", "data"]) == [
        "polyoxide-venue-crate: module `data`'s README.md is included 0 times under "
        "`cfg(all(doctest, feature = \"data\"))`, not once"]


def test_an_include_without_a_readme_is_caught(tmp_path: Path) -> None:
    crate = _crate(tmp_path, GAMMA + DATA, ["gamma"])
    assert problems(crate, ["gamma", "data"]) == [
        "polyoxide-venue-crate: module `data`'s include in src/lib.rs names "
        "data/README.md, which is not a file"]


def test_a_readme_under_another_modules_feature_is_caught(tmp_path: Path) -> None:
    crossed = GAMMA + DATA.replace('feature = "data"', 'feature = "gamma"')
    crate = _crate(tmp_path, crossed, ["gamma", "data"])
    assert problems(crate, ["gamma", "data"]) == [
        "polyoxide-venue-crate: module `data`'s README.md is included 0 times under "
        "`cfg(all(doctest, feature = \"data\"))`, not once"]


def test_a_readme_included_twice_is_caught(tmp_path: Path) -> None:
    crate = _crate(tmp_path, GAMMA + GAMMA.replace("GammaReadme", "OtherReadme"), ["gamma"])
    assert problems(crate, ["gamma"]) == [
        "polyoxide-venue-crate: module `gamma`'s README.md is included 2 times under "
        "`cfg(all(doctest, feature = \"gamma\"))`, not once"]


def test_an_include_in_a_module_file_resolves_against_it(tmp_path: Path) -> None:
    crate = _crate(tmp_path, "pub mod gamma;\n", ["gamma"])
    (crate / "src" / "gamma" / "mod.rs").write_text(GAMMA.replace("gamma/README.md", "README.md"))
    assert problems(crate, ["gamma"]) == []


def test_an_ungated_include_does_not_count(tmp_path: Path) -> None:
    lib = '#[cfg(doctest)]\n#[doc = include_str!("gamma/README.md")]\nstruct Readme;\n'
    crate = _crate(tmp_path, lib, ["gamma"])
    assert problems(crate, ["gamma"]) == [
        "polyoxide-venue-crate: module `gamma`'s README.md is included 0 times under "
        "`cfg(all(doctest, feature = \"gamma\"))`, not once"]


def test_a_module_directory_without_its_readme_is_caught(tmp_path: Path) -> None:
    """Deleting a module's README with its include leaves nothing to count, so the
    directory is what demands one."""
    crate = _crate(tmp_path, GAMMA, ["gamma"])
    (crate / "src" / "data").mkdir()
    (crate / "src" / "data" / "mod.rs").write_text("pub mod api;\n")
    assert problems(crate, ["gamma", "data"]) == [
        "polyoxide-venue-crate: module `data` has no README.md in src/data/"]


def test_a_product_without_a_module_directory_is_not_checked(tmp_path: Path) -> None:
    """A crate that is still its one module, as perps and clob are, keeps its README
    at the crate root."""
    crate = _crate(tmp_path, "", [])
    assert problems(crate, ["perps"]) == []


def test_only_members_with_a_venue_and_products_are_read() -> None:
    def package(name: str, features=(), **meta) -> dict:
        return {"id": name, "name": name, "manifest_path": f"/ws/{name}/Cargo.toml",
                "features": {f: [] for f in features},
                "metadata": {"polyoxide": meta} if meta else None}

    packages = [
        package("polyoxide-core"),
        package("polyoxide-polymarket", ["gamma", "data", "clob", "specta"],
                venue="polymarket", products=["gamma", "data"]),
        package("polyoxide-kalshi", venue="kalshi"),
        package("polyoxide-binance", ["ws"], venue="binance", products=["usdm"]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    assert venue_modules(metadata) == [(Path("/ws/polyoxide-polymarket"), ["gamma", "data"])]
