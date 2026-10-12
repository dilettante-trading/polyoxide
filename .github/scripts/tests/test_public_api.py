"""Unit tests for scripts/public_api.py.

The walker runs on synthetic rustdoc JSON, built here with only the fields it
reads, in the shape format 57 gives them. The builder runs against a stub runner,
and the extraction against a git repository made in a temporary directory, so
nothing here builds a crate.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]


def _load(name: str):
    """A module from `scripts/`, which lives outside this uv project."""
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


public_api = _load("public_api")
PublicApiError = public_api.PublicApiError

HIDDEN = [{"other": "#[doc(hidden)]"}]


# --- synthetic rustdoc JSON --------------------------------------------------


class Doc:
    """One crate's rustdoc JSON, as the walker reads it."""

    def __init__(self, name: str, version: int = 57) -> None:
        self.name = name
        self.version = version
        self.index: dict[str, dict] = {}
        self.paths: dict[str, dict] = {}
        self.external: dict[str, dict] = {}
        self.next = 0
        self.root = self._add(name, {"module": {"is_crate": True, "items": [],
                                                "is_stripped": False}})
        self.paths[str(self.root)] = {"crate_id": 0, "path": [name], "kind": "module"}

    def _add(self, name: str | None, inner, visibility: str = "public", attrs=()) -> int:
        id_ = self.next
        self.next += 1
        self.index[str(id_)] = {"id": id_, "crate_id": 0, "name": name,
                                "visibility": visibility, "attrs": list(attrs), "inner": inner}
        return id_

    def _child(self, parent: int, id_: int) -> int:
        self.index[str(parent)]["inner"]["module"]["items"].append(id_)
        return id_

    def _local(self, id_: int, path: list[str], kind: str) -> None:
        self.paths[str(id_)] = {"crate_id": 0, "path": path, "kind": kind}

    def module(self, parent: int, name: str, visibility: str = "public", attrs=(),
               path: list[str] | None = None) -> int:
        id_ = self._add(name, {"module": {"is_crate": False, "items": [], "is_stripped": False}},
                        visibility, attrs)
        self._local(id_, path or [self.name, name], "module")
        return self._child(parent, id_)

    def function(self, parent: int | None, name: str, visibility: str = "public", attrs=(),
                 path: list[str] | None = None) -> int:
        id_ = self._add(name, {"function": {}}, visibility, attrs)
        if path:
            self._local(id_, path, "function")
        return self._child(parent, id_) if parent is not None else id_

    def macro(self, parent: int, name: str) -> int:
        return self._child(parent, self._add(name, {"macro": "macro_rules! x {}"}))

    def field(self, name: str, visibility: str = "public", attrs=()) -> int:
        return self._add(name, {"struct_field": {"primitive": "u8"}}, visibility, attrs)

    def impl(self, items: list[int], trait=None, blanket=None, synthetic=False, attrs=()) -> int:
        return self._add(None, {"impl": {"items": items, "trait": trait, "blanket_impl": blanket,
                                         "is_synthetic": synthetic}}, "default", attrs)

    def struct(self, parent: int, name: str, fields: list[int] = (), impls: list[int] = (),
               tuple_fields: list[int | None] | None = None, attrs=(),
               path: list[str] | None = None) -> int:
        shape = ({"tuple": tuple_fields} if tuple_fields is not None
                 else {"plain": {"fields": list(fields), "has_stripped_fields": False}})
        id_ = self._add(name, {"struct": {"kind": shape, "generics": {}, "impls": list(impls)}},
                        attrs=attrs)
        self._local(id_, path or [self.name, name], "struct")
        return self._child(parent, id_)

    def variant(self, name: str, fields: list[int] | None = None, attrs=(),
                tuple_fields: list[int | None] | None = None) -> int:
        shape = ({"tuple": tuple_fields} if tuple_fields is not None
                 else "plain" if fields is None
                 else {"struct": {"fields": fields, "has_stripped_fields": False}})
        return self._add(name, {"variant": {"kind": shape, "discriminant": None}},
                         "default", attrs)

    def enum(self, parent: int, name: str, variants: list[int], impls: list[int] = ()) -> int:
        id_ = self._add(name, {"enum": {"variants": variants, "generics": {},
                                        "impls": list(impls)}})
        self._local(id_, [self.name, name], "enum")
        return self._child(parent, id_)

    def trait(self, parent: int, name: str, items: list[int]) -> int:
        id_ = self._add(name, {"trait": {"items": items, "implementations": []}})
        return self._child(parent, id_)

    def alias(self, parent: int, name: str, target: int, target_path: str) -> int:
        named = {"resolved_path": {"path": target_path, "id": target, "args": None}}
        id_ = self._add(name, {"type_alias": {"type": named, "generics": {}}})
        return self._child(parent, id_)

    def use(self, parent: int, source: str, name: str, target: int | None, glob: bool = False,
            attrs=()) -> int:
        id_ = self._add(None, {"use": {"source": source, "name": name, "id": target,
                                       "is_glob": glob}}, attrs=attrs)
        return self._child(parent, id_)

    def foreign(self, crate: str, path: list[str], kind: str) -> int:
        """An id for another crate's item, known to this one only through `paths`."""
        crate_id = next((k for k, v in self.external.items() if v["name"] == crate), None)
        if crate_id is None:
            crate_id = str(100 + len(self.external))
            self.external[crate_id] = {"name": crate}
        id_ = 10_000 + self.next
        self.next += 1
        self.paths[str(id_)] = {"crate_id": int(crate_id), "path": path, "kind": kind}
        return id_

    def json(self) -> dict:
        return {"root": self.root, "crate_version": "0.0.0", "includes_private": False,
                "index": self.index, "paths": self.paths, "external_crates": self.external,
                "format_version": self.version}

    def write(self, directory: Path) -> None:
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"{self.name}.json").write_text(json.dumps(self.json()))


def listing(directory: Path, *libraries: str) -> dict:
    return public_api.walk(directory, list(libraries))


# --- the walker --------------------------------------------------------------


def a_crate() -> Doc:
    """`polyoxide_a` with one of each thing the walker lists."""
    doc = Doc("polyoxide_a")
    root = doc.root
    new = doc.function(None, "new")
    private_method = doc.function(None, "secret", visibility="default")
    hidden_method = doc.function(None, "for_tests", attrs=HIDDEN)
    trait_method = doc.function(None, "clone")
    blanket_method = doc.function(None, "borrow")
    auto_method = doc.function(None, "auto")
    doc.struct(root, "Thing",
               fields=[doc.field("x"), doc.field("y", visibility="default"),
                       doc.field("z", attrs=HIDDEN)],
               impls=[doc.impl([new, private_method, hidden_method]),
                      doc.impl([trait_method], trait={"path": "Clone", "id": 1}),
                      doc.impl([blanket_method], blanket={"generic": "T"}),
                      doc.impl([auto_method], synthetic=True)])
    doc.struct(root, "Pair", tuple_fields=[doc.field("0"), doc.field("1", visibility="default")])
    doc.enum(root, "Mode", [doc.variant("Plain"), doc.variant("Shaped", [doc.field("f", "default")]),
                            doc.variant("Pair", tuple_fields=[doc.field("0", "default"),
                                                              doc.field("1", "default")])])
    doc.trait(root, "Tr", [doc.function(None, "required")])
    doc.macro(root, "mac")
    tests = doc.module(root, "fixtures", attrs=HIDDEN)
    doc.function(tests, "SOCCER", path=["polyoxide_a", "fixtures", "SOCCER"])
    helper = doc.function(None, "helper", path=["polyoxide_a", "private", "helper"])
    doc.use(root, "private::helper", "hidden_helper", helper, attrs=HIDDEN)
    secret = doc.function(None, "secret_fn", attrs=HIDDEN,
                          path=["polyoxide_a", "private", "secret_fn"])
    doc.use(root, "private::secret_fn", "secret_fn", secret)
    doc.module(root, "internal", visibility="crate")
    return doc


def test_each_kind_of_public_path_is_listed(tmp_path: Path) -> None:
    a_crate().write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    assert set(paths) == {
        ("module", "polyoxide_a"),
        ("struct", "polyoxide_a::Thing"),
        ("struct_field", "polyoxide_a::Thing::x"),
        ("struct_field", "polyoxide_a::Thing::z"),
        ("function", "polyoxide_a::Thing::new"),
        ("function", "polyoxide_a::Thing::for_tests"),
        ("struct", "polyoxide_a::Pair"),
        ("struct_field", "polyoxide_a::Pair::0"),
        ("enum", "polyoxide_a::Mode"),
        ("variant", "polyoxide_a::Mode::Plain"),
        ("variant", "polyoxide_a::Mode::Shaped"),
        ("struct_field", "polyoxide_a::Mode::Shaped::f"),
        ("variant", "polyoxide_a::Mode::Pair"),
        ("struct_field", "polyoxide_a::Mode::Pair::0"),
        ("struct_field", "polyoxide_a::Mode::Pair::1"),
        ("trait", "polyoxide_a::Tr"),
        ("trait_function", "polyoxide_a::Tr::required"),
        ("macro", "polyoxide_a::mac"),
        ("module", "polyoxide_a::fixtures"),
        ("function", "polyoxide_a::fixtures::SOCCER"),
        ("function", "polyoxide_a::hidden_helper"),
        ("function", "polyoxide_a::secret_fn"),
    }


def test_trait_blanket_and_synthetic_impls_are_skipped(tmp_path: Path) -> None:
    a_crate().write(tmp_path)
    names = {path.split("::")[-1] for _, path in listing(tmp_path, "polyoxide_a")}
    assert not names & {"clone", "borrow", "auto", "secret", "y", "internal"}


def test_hidden_paths_are_listed_and_flagged(tmp_path: Path) -> None:
    """An item is hidden when it, a `use` naming it, or anything on its path is."""
    a_crate().write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    hidden = {path for (_, path), flag in paths.items() if flag}
    assert hidden == {"polyoxide_a::fixtures", "polyoxide_a::fixtures::SOCCER",
                      "polyoxide_a::Thing::for_tests", "polyoxide_a::Thing::z",
                      "polyoxide_a::hidden_helper", "polyoxide_a::secret_fn"}


def test_a_path_reached_both_ways_is_hidden_only_if_every_way_is(tmp_path: Path) -> None:
    doc = Doc("polyoxide_a")
    target = doc.function(None, "f", path=["polyoxide_a", "inner", "f"])
    doc.use(doc.root, "inner::f", "f", target, attrs=HIDDEN)
    open_module = doc.module(doc.root, "open")
    doc.use(open_module, "crate::*", "*", doc.root, glob=True)
    doc.write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    assert paths[("function", "polyoxide_a::f")] is True
    assert paths[("function", "polyoxide_a::open::f")] is True
    # A hidden re-export still lists every path a consumer can import.
    assert ("module", "polyoxide_a::open") in paths


def test_a_path_reached_hidden_and_not_is_not_hidden(tmp_path: Path) -> None:
    """Two globs bring one item to one path, the second through a hidden `use`."""
    doc = Doc("polyoxide_a")
    target = doc.function(None, "f", path=["polyoxide_a", "inner", "f"])
    one = doc.module(doc.root, "one")
    doc.use(one, "crate::inner::f", "f", target)
    two = doc.module(doc.root, "two")
    doc.use(two, "crate::inner::f", "f", target)
    doc.use(doc.root, "one::*", "*", one, glob=True)
    doc.use(doc.root, "two::*", "*", two, glob=True, attrs=HIDDEN)
    doc.write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    assert paths[("function", "polyoxide_a::f")] is False


def umbrella(with_sibling: bool, tmp_path: Path) -> dict:
    """`polyoxide` re-exporting a whole sibling and one of its items, as the umbrella
    does, and an alias of the sibling's generic type."""
    sibling = Doc("polyoxide_a")
    method = sibling.function(None, "membership")
    sibling.struct(sibling.root, "Supervised", fields=[sibling.field("queue")],
                   impls=[sibling.impl([method])],
                   path=["polyoxide_a", "supervisor", "Supervised"])
    ws = sibling.module(sibling.root, "ws")
    sibling.function(ws, "connect", path=["polyoxide_a", "ws", "connect"])
    top = Doc("polyoxide")
    whole = top.foreign("polyoxide_a", ["polyoxide_a"], "module")
    top.use(top.root, "polyoxide_a", "polyoxide_a", whole)
    item = top.foreign("polyoxide_a", ["polyoxide_a", "supervisor", "Supervised"], "struct")
    top.use(top.root, "polyoxide_a::Supervised", "Supervised", item)
    top.alias(top.root, "SupervisedA", item, "polyoxide_a::Supervised")
    top.write(tmp_path)
    if with_sibling:
        sibling.write(tmp_path)
    return listing(tmp_path, "polyoxide")


def test_cross_crate_re_exports_are_followed_through_the_siblings_json(tmp_path: Path) -> None:
    paths = umbrella(True, tmp_path)
    for key in [("module", "polyoxide::polyoxide_a"),
                ("module", "polyoxide::polyoxide_a::ws"),
                ("function", "polyoxide::polyoxide_a::ws::connect"),
                ("struct", "polyoxide::Supervised"),
                ("function", "polyoxide::Supervised::membership"),
                ("struct_field", "polyoxide::Supervised::queue")]:
        assert key in paths, key


def test_members_reached_through_a_type_alias_are_listed(tmp_path: Path) -> None:
    paths = umbrella(True, tmp_path)
    assert paths.keys() >= {("type_alias", "polyoxide::SupervisedA"),
                            ("function", "polyoxide::SupervisedA::membership"),
                            ("struct_field", "polyoxide::SupervisedA::queue")}


def test_a_crate_with_no_json_is_listed_once_with_no_members(tmp_path: Path) -> None:
    paths = umbrella(False, tmp_path)
    assert ("module", "polyoxide::polyoxide_a") in paths
    assert ("struct", "polyoxide::Supervised") in paths
    assert not [path for _, path in paths if path.startswith("polyoxide::polyoxide_a::")]
    assert ("type_alias", "polyoxide::SupervisedA") in paths
    assert not [path for _, path in paths if path.startswith("polyoxide::SupervisedA::")]


def test_an_ancestor_alias_is_listed_and_not_entered(tmp_path: Path) -> None:
    """`pub use crate as root` and `pub use super::me as me` would list forever."""
    doc = Doc("polyoxide_a")
    me = doc.module(doc.root, "me")
    doc.function(me, "f")
    doc.use(me, "crate", "root", doc.root)
    doc.use(me, "super::me", "again", me)
    doc.write(tmp_path)
    paths = {path for _, path in listing(tmp_path, "polyoxide_a")}
    assert paths == {"polyoxide_a", "polyoxide_a::me", "polyoxide_a::me::f",
                     "polyoxide_a::me::root", "polyoxide_a::me::again"}


def test_glob_cycles_end(tmp_path: Path) -> None:
    doc = Doc("polyoxide_a")
    a = doc.module(doc.root, "a")
    b = doc.module(doc.root, "b")
    doc.use(a, "super::b::*", "*", b, glob=True)
    doc.use(b, "super::a::*", "*", a, glob=True)
    doc.struct(a, "InA", path=["polyoxide_a", "a", "InA"])
    doc.struct(b, "InB", path=["polyoxide_a", "b", "InB"])
    doc.write(tmp_path)
    paths = {path for _, path in listing(tmp_path, "polyoxide_a")}
    assert {"polyoxide_a::a::InA", "polyoxide_a::a::InB",
            "polyoxide_a::b::InA", "polyoxide_a::b::InB"} <= paths


def test_an_explicit_name_shadows_a_glob_in_its_namespace(tmp_path: Path) -> None:
    doc = Doc("polyoxide_a")
    inner = doc.module(doc.root, "inner")
    doc.function(inner, "helper", path=["polyoxide_a", "inner", "helper"])
    doc.struct(inner, "Kept", path=["polyoxide_a", "inner", "Kept"])
    shadow = doc.module(doc.root, "shadow")
    doc.use(shadow, "crate::inner::*", "*", inner, glob=True)
    doc.function(shadow, "helper", path=["polyoxide_a", "shadow", "helper"])
    doc.write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    assert ("struct", "polyoxide_a::shadow::Kept") in paths
    assert [k for k in paths if k[1] == "polyoxide_a::shadow::helper"] == [
        ("function", "polyoxide_a::shadow::helper")]


def test_an_enum_glob_lists_its_variants(tmp_path: Path) -> None:
    doc = Doc("polyoxide_a")
    mode = doc.enum(doc.root, "Mode", [doc.variant("On"), doc.variant("Off")])
    globbed = doc.module(doc.root, "modes")
    doc.use(globbed, "crate::Mode::*", "*", mode, glob=True)
    doc.write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    assert {("variant", "polyoxide_a::modes::On"), ("variant", "polyoxide_a::modes::Off")} <= set(paths)


def test_a_module_and_a_function_of_one_name_are_two_paths(tmp_path: Path) -> None:
    doc = Doc("polyoxide_a")
    doc.module(doc.root, "and_fn")
    doc.function(doc.root, "and_fn")
    doc.write(tmp_path)
    paths = listing(tmp_path, "polyoxide_a")
    assert {("module", "polyoxide_a::and_fn"), ("function", "polyoxide_a::and_fn")} <= set(paths)


@pytest.mark.parametrize("version", [57, 60, 61])
def test_the_three_formats_are_read(tmp_path: Path, version: int) -> None:
    doc = Doc("polyoxide_a", version=version)
    doc.function(doc.root, "f")
    doc.write(tmp_path)
    assert ("function", "polyoxide_a::f") in listing(tmp_path, "polyoxide_a")


@pytest.mark.parametrize("version", [56, 58, 59, 62, None])
def test_another_format_is_refused(tmp_path: Path, version) -> None:
    doc = Doc("polyoxide_a", version=version)
    doc.write(tmp_path)
    with pytest.raises(PublicApiError, match=f"format {version}") as raised:
        listing(tmp_path, "polyoxide_a")
    assert raised.value.exit_code == 2


def test_a_sibling_in_another_format_is_refused_too(tmp_path: Path) -> None:
    umbrella(True, tmp_path)
    data = json.loads((tmp_path / "polyoxide_a.json").read_text())
    data["format_version"] = 99
    (tmp_path / "polyoxide_a.json").write_text(json.dumps(data))
    with pytest.raises(PublicApiError, match="format 99"):
        listing(tmp_path, "polyoxide")


def test_a_missing_crate_is_refused(tmp_path: Path) -> None:
    with pytest.raises(PublicApiError, match="no rustdoc JSON for polyoxide_a"):
        listing(tmp_path, "polyoxide_a")


# --- the workspace -----------------------------------------------------------


def package(name: str, publish=None, kinds=("lib",), features=None) -> dict:
    return {"name": name, "id": name, "publish": publish,
            "manifest_path": f"/w/{name}/Cargo.toml",
            "targets": [{"kind": list(kinds), "name": name}],
            "features": features or {}}


def metadata(*packages: dict) -> dict:
    return {"packages": list(packages), "workspace_members": [p["id"] for p in packages],
            "target_directory": "/w/target"}


def test_only_publishable_libraries_are_built() -> None:
    found = public_api.library_packages(metadata(
        package("polyoxide-a"), package("polyoxide-py", publish=[]),
        package("polyoxide-bin", kinds=("bin",)), package("polyoxide-cli", kinds=("lib", "bin")),
        package("polyoxide-b", publish=["crates-io"])))
    assert [p["name"] for p in found] == ["polyoxide-a", "polyoxide-cli", "polyoxide-b"]


def test_features_are_keyed_by_package() -> None:
    found = public_api.features(metadata(
        package("polyoxide-clob", features={"default": ["gamma"], "gamma": [], "ws": []}),
        package("polyoxide-py", publish=[], features={"x": []})))
    assert found == {"polyoxide-clob/default", "polyoxide-clob/gamma", "polyoxide-clob/ws"}


# --- the builder -------------------------------------------------------------


class Stub:
    """Answers each command by its leading words, recording every call."""

    def __init__(self, answers: dict) -> None:
        self.answers = answers
        self.calls: list[list[str]] = []

    def __call__(self, command: list[str]) -> subprocess.CompletedProcess:
        self.calls.append(command)
        for prefix, answer in self.answers.items():
            if tuple(command[: len(prefix)]) == prefix:
                code, out = answer(command) if callable(answer) else answer
                return subprocess.CompletedProcess(command, code, out, "")
        raise AssertionError(f"unexpected command {command}")

    def called(self, *prefix: str) -> list[list[str]]:
        return [c for c in self.calls if tuple(c[: len(prefix)]) == prefix]


def rustdoc(target: Path):
    """A `cargo rustdoc` stub that writes a JSON file for the package it was given."""
    def answer(command: list[str]) -> tuple[int, str]:
        name = command[command.index("-p") + 1].replace("-", "_")
        (target / "doc").mkdir(parents=True, exist_ok=True)
        (target / "doc" / f"{name}.json").write_text(json.dumps(Doc(name).json()))
        return 0, ""
    return answer


def build(tmp_path: Path, stream: Stub, source: str = "tree-1", rustc: str = "rustc 1.99.0"):
    packages = [package("polyoxide-a"), package("polyoxide-b")]
    return public_api.build(tmp_path / "tree", packages, tmp_path / "json", tmp_path / "target",
                            source, rustc, stream)


def test_each_crate_is_built_with_all_features_and_hidden_items(tmp_path: Path) -> None:
    stream = Stub({("env",): rustdoc(tmp_path / "target")})
    assert build(tmp_path, stream) == ["polyoxide_a", "polyoxide_b"]
    assert stream.calls[0] == [
        "env", "RUSTC_BOOTSTRAP=1", "cargo", "rustdoc",
        "--manifest-path", str(tmp_path / "tree" / "Cargo.toml"), "-p", "polyoxide-a",
        "--all-features", "--lib", "--locked", "--target-dir", str(tmp_path / "target"),
        "--color", "never", "--", "-Z", "unstable-options", "--output-format", "json",
        "--document-hidden-items"]
    assert len(stream.calls) == 2
    assert (tmp_path / "json" / "polyoxide_b.json").is_file()


def test_the_json_is_reused_while_the_tree_and_toolchain_match(tmp_path: Path) -> None:
    stream = Stub({("env",): rustdoc(tmp_path / "target")})
    build(tmp_path, stream)
    build(tmp_path, stream)
    assert len(stream.calls) == 2
    build(tmp_path, stream, rustc="rustc 1.99.1")
    assert len(stream.calls) == 4
    build(tmp_path, stream, rustc="rustc 1.99.1", source="tree-2")
    assert len(stream.calls) == 6


def test_a_stamp_from_another_rustdoc_command_is_rebuilt(tmp_path: Path) -> None:
    """A change to how the JSON is built, such as a flag dropped, outdates it as a new
    toolchain does."""
    stream = Stub({("env",): rustdoc(tmp_path / "target")})
    build(tmp_path, stream)
    stamp_path = tmp_path / "json" / public_api.STAMP
    stamp = json.loads(stamp_path.read_text())
    assert "--document-hidden-items" in stamp["command"]
    stamp["command"].remove("--document-hidden-items")
    stamp_path.write_text(json.dumps(stamp))
    build(tmp_path, stream)
    assert len(stream.calls) == 4


def test_a_lost_json_file_is_rebuilt(tmp_path: Path) -> None:
    stream = Stub({("env",): rustdoc(tmp_path / "target")})
    build(tmp_path, stream)
    (tmp_path / "json" / "polyoxide_a.json").unlink()
    build(tmp_path, stream)
    assert len(stream.calls) == 4


def test_a_failed_build_cannot_decide_and_leaves_no_stamp(tmp_path: Path) -> None:
    stream = Stub({("env",): (101, "")})
    with pytest.raises(PublicApiError, match="exited 101"):
        build(tmp_path, stream)
    assert not (tmp_path / "json" / public_api.STAMP).exists()


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "-q")
    (repo / "Cargo.toml").write_text("[workspace]\n")
    git(repo, "add", ".")
    git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "one")
    git(repo, "tag", "v1")
    return repo


def in_repo(repo: Path):
    def run(command: list[str]) -> subprocess.CompletedProcess:
        return subprocess.run(command, cwd=repo, capture_output=True, text=True)
    return run


def test_a_revision_is_extracted_once_per_tree(repo: Path, tmp_path: Path) -> None:
    run = in_repo(repo)
    tree_hash = public_api.revision_hash("v1", run)
    tree = public_api.extract("v1", tree_hash, tmp_path / "scratch", run)
    assert tree == tmp_path / "scratch" / "baseline" / "tree"
    assert (tree / "Cargo.toml").read_text() == "[workspace]\n"
    (tree / "marker").write_text("kept")
    public_api.extract("v1", tree_hash, tmp_path / "scratch", run)
    assert (tree / "marker").is_file()
    public_api.extract("v1", "another", tmp_path / "scratch", run)
    assert not (tree / "marker").exists()


def test_an_extraction_cut_short_is_never_reused(repo: Path, tmp_path: Path) -> None:
    run = in_repo(repo)
    tree_hash = public_api.revision_hash("v1", run)
    scratch = tmp_path / "scratch"
    tree = public_api.extract("v1", tree_hash, scratch, run)
    failing = Stub({("git", "archive"): (128, "")})
    with pytest.raises(PublicApiError, match="exited 128"):
        public_api.extract("v1", "another", scratch, failing)
    assert not (scratch / "baseline" / "tree.stamp").exists()
    public_api.extract("v1", tree_hash, scratch, run)
    assert (tree / "Cargo.toml").is_file()


def test_the_working_trees_hash_follows_an_uncommitted_edit(repo: Path) -> None:
    run = in_repo(repo)
    before = public_api.worktree_hash(repo, run)
    assert public_api.worktree_hash(repo, run) == before
    (repo / "new.rs").write_text("pub fn f() {}\n")
    after = public_api.worktree_hash(repo, run)
    assert after != before
    (repo / "Cargo.toml").write_text("[workspace]\nmembers = []\n")
    assert public_api.worktree_hash(repo, run) != after


def test_a_snapshot_builds_its_side_and_reads_its_features(tmp_path: Path) -> None:
    """The working tree's JSON and the baseline's never share a directory, and the
    two trees share one target directory."""
    scratch = tmp_path / "scratch"
    meta = metadata(package("polyoxide-a", features={"ws": []}))
    run = Stub({("rustc", "-vV"): (0, "rustc 1.99.0\n"),
                ("git", "-C"): (0, "Cargo.toml\0"),
                ("cargo", "metadata"): (0, json.dumps(meta))})
    stream = Stub({("env",): rustdoc(scratch / "build")})
    (tmp_path / "Cargo.toml").write_text("[workspace]\n")
    found = public_api.snapshot(None, tmp_path, scratch, run, stream)
    assert found.paths == {("module", "polyoxide_a"): False}
    assert found.features == {"polyoxide-a/ws"}
    assert (scratch / "head" / "json" / "polyoxide_a.json").is_file()
    assert stream.calls[0][stream.calls[0].index("--target-dir") + 1] == str(scratch / "build")


def test_a_revisions_snapshot_is_built_and_read_from_its_own_tree(repo: Path,
                                                                  tmp_path: Path) -> None:
    """The baseline's crates exist only in its own tree, so its metadata, every
    build and its JSON must come from `<scratch>/baseline/`, never the working tree."""
    scratch = tmp_path / "scratch"
    tree = scratch / "baseline" / "tree"
    meta = metadata(package("polyoxide-old", features={"ws": []}))
    git_run = in_repo(repo)

    def through_git(command: list[str]) -> tuple[int, str]:
        result = git_run(command)
        return result.returncode, result.stdout

    run = Stub({("rustc", "-vV"): (0, "rustc 1.99.0\n"),
                ("git",): through_git,
                ("cargo", "metadata"): (0, json.dumps(meta))})
    stream = Stub({("env",): rustdoc(scratch / "build")})
    found = public_api.snapshot("v1", repo, scratch, run, stream)
    [command] = run.called("cargo", "metadata")
    assert command[command.index("--manifest-path") + 1] == str(tree / "Cargo.toml")
    assert [c[c.index("--manifest-path") + 1] for c in stream.calls] == [
        str(tree / "Cargo.toml")]
    assert (scratch / "baseline" / "json" / "polyoxide_old.json").is_file()
    assert not (scratch / "head").exists()
    assert found.paths == {("module", "polyoxide_old"): False}
    assert found.features == {"polyoxide-old/ws"}
