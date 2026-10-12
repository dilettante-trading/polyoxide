#!/usr/bin/env python3
"""Every public path of the workspace's crates, read from their rustdoc JSON.

The S2 removal gate (`api_removals.py manifest`) diffs two of these listings: the
baseline's and the working tree's. A listing maps each path to whether it is
`#[doc(hidden)]`, keyed by `(kind, path)`, since a module and a function may share
a path.

What a listing holds, for each publishable member with a library target:

* modules, types, traits, functions, constants, statics, type aliases and the
  macros `#[macro_export]` puts at the crate root;
* enum variants and their fields, public struct and union fields (a tuple
  field, a tuple variant's included, by its index), inherent methods and associated
  constants, and a trait's own items;
* the members of a type reached through a type alias, under the alias's path;
* every path a public `use` makes importable, renamed, glob or across crates.
  A sibling crate's item is only in the re-exporting crate's `paths`, so the
  walker opens the sibling's JSON and goes on there; an item of a crate with no
  JSON (reqwest, tungstenite) is listed once, with no members.

Trait impls, blanket impls and synthetic (auto-trait) impls are skipped. A module
already on the current path, such as a `pub use crate as root`, is listed but not
entered again, and so is a glob already being expanded. `#[doc(hidden)]` items are
listed and flagged: an item is hidden when it, a `use` naming it, or anything on
its path is.

The JSON is built with

    RUSTC_BOOTSTRAP=1 cargo rustdoc -p <crate> --all-features --lib --locked \\
        -- -Z unstable-options --output-format json --document-hidden-items

into a target directory of its own under `target/api-removals/build/`, so the
workspace's own builds never see `RUSTC_BOOTSTRAP`. A revision is extracted with
`git archive` into `target/api-removals/baseline/tree/`, since a crate the
working tree no longer has can only be built from its own tree. The JSON is kept
under `target/api-removals/<baseline|head>/json/` with a stamp naming the source
tree's hash, `rustc -vV` and the rustdoc command, and reused while all three
match. A `format_version` other
than 57, 60 or 61 is refused: those are the ones cargo-semver-checks 0.51.0 reads,
which Rust 1.93 to 1.99 emit (1.95 emits 57), and the fields read here are the same
in each. Cargo reads its config from the current directory, the repository, so a
baseline holding a `.cargo/config.toml` of its own would be built without it;
v0.39.0 holds none.

Stdlib only.

Usage:
    python3 scripts/public_api.py list [--rev REV]
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tarfile
from collections.abc import Callable
from pathlib import Path
from typing import NamedTuple

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish_order  # noqa: E402

REPO = publish_order.REPO
FORMAT_VERSIONS = (57, 60, 61)
SCRATCH = "api-removals"
STAMP = "stamp.json"

HIDDEN = re.compile(r"#\[doc\((?:[^)]*[\s,(])?hidden\b")

# The namespace each kind lives in, for glob shadowing: an explicit name hides a
# glob-imported name only in the same namespace.
TYPE_NS = {"module", "struct", "enum", "union", "trait", "trait_alias", "type_alias",
           "extern_crate", "primitive", "extern_type"}
VALUE_NS = {"function", "constant", "static", "variant"}
MACRO_NS = {"macro", "proc_macro", "proc_attribute", "proc_derive"}

# A runner takes a command and returns the finished process, its output as text.
Runner = Callable[[list], subprocess.CompletedProcess]
# A listing: (kind, path) -> whether the path is doc-hidden.
Paths = dict[tuple[str, str], bool]


class PublicApiError(Exception):
    """The listing cannot be made. The message says why."""

    exit_code = 2


class Snapshot(NamedTuple):
    paths: Paths
    # `<package>/<feature>` for each feature of each publishable member.
    features: frozenset[str]


# --- reading rustdoc JSON ----------------------------------------------------


def is_hidden(item: dict | None) -> bool:
    if not item:
        return False
    for attr in item.get("attrs", ()):
        text = attr.get("other") if isinstance(attr, dict) else attr
        if isinstance(text, str) and HIDDEN.search(text):
            return True
    return False


def kind_of(item: dict) -> str:
    inner = item["inner"]
    kind = next(iter(inner)) if isinstance(inner, dict) else inner
    if kind == "proc_macro":
        return {"bang": "macro", "attr": "proc_attribute", "derive": "proc_derive"}.get(
            inner["proc_macro"]["kind"], "proc_macro")
    return kind


def namespaces(kind: str, item: dict | None) -> set[str]:
    if kind in MACRO_NS:
        return {"macro"}
    if kind in VALUE_NS:
        return {"value"}
    if kind == "struct" and item is not None:
        shape = item["inner"]["struct"]["kind"]
        return {"type", "value"} if shape == "unit" or "tuple" in shape else {"type"}
    if kind in TYPE_NS:
        return {"type"}
    return {kind}


class Crate:
    """One crate's rustdoc JSON."""

    def __init__(self, data: dict, label: str):
        version = data.get("format_version")
        if version not in FORMAT_VERSIONS:
            raise PublicApiError(
                f"{label} is rustdoc JSON format {version}, and only formats "
                f"{', '.join(map(str, FORMAT_VERSIONS))} are read; the fields the walker "
                f"reads may have changed")
        self.index: dict = data["index"]
        self.paths: dict = data["paths"]
        self.external: dict = data["external_crates"]
        self.root = data["root"]
        self.name = self.index[str(self.root)]["name"]
        self._by_path: dict[tuple, list] | None = None

    @classmethod
    def load(cls, path: Path) -> Crate:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as err:
            raise PublicApiError(f"could not read {path}: {err}") from err
        return cls(data, str(path))

    def item(self, id_) -> dict | None:
        return None if id_ is None else self.index.get(str(id_))

    def local_by_path(self, path: tuple, kind: str | None = None) -> int | None:
        """This crate's id for a canonical path from another crate's `paths`.

        `paths` lists every local item by its definition path, through private
        modules too, so it serves to find a foreign id and never to list a module.
        A module and a function may share a path, so the kind is matched too.
        """
        if self._by_path is None:
            self._by_path = collections.defaultdict(list)
            for key, summary in self.paths.items():
                if summary["crate_id"] == 0:
                    self._by_path[tuple(summary["path"])].append((summary["kind"], int(key)))
            self._by_path[(self.name,)].append(("module", self.root))
        found = self._by_path.get(path, [])
        for found_kind, id_ in found:
            if kind is None or found_kind == kind:
                return id_
        return found[0][1] if found else None


class External(NamedTuple):
    """An item known only by its `paths` summary, in a crate with no JSON here."""

    path: tuple[str, ...]
    kind: str


Loader = Callable[[str], Crate | None]


def directory_loader(directory: Path) -> Loader:
    """Each crate's JSON from `<directory>/<library name>.json`, read once."""
    loaded: dict[str, Crate | None] = {}

    def load(name: str) -> Crate | None:
        if name not in loaded:
            path = directory / f"{name}.json"
            loaded[name] = Crate.load(path) if path.is_file() else None
        return loaded[name]
    return load


class Walker:
    """Lists one crate's public paths, following re-exports into siblings."""

    def __init__(self, crate: Crate, load: Loader):
        self.top = crate
        self.load = load
        self.out: Paths = {}

    def crate(self, name: str) -> Crate | None:
        return self.top if name == self.top.name else self.load(name)

    def resolve(self, crate: Crate, id_):
        """An id `crate` names, as (crate, id) in the crate whose index holds it, or
        as External, or None when `crate` does not know it."""
        if crate.item(id_) is not None:
            return crate, int(id_)
        summary = crate.paths.get(str(id_))
        if summary is None:
            return None
        if summary["crate_id"] != 0:
            other = self.crate(crate.external[str(summary["crate_id"])]["name"])
            if other is not None:
                found = other.local_by_path(tuple(summary["path"]), summary["kind"])
                if found is not None and other.item(found) is not None:
                    return other, found
        return External(tuple(summary["path"]), summary["kind"])

    # -- module members, globs expanded ---------------------------------------

    def members(self, crate: Crate, module: int, globbing: frozenset) -> list:
        """(name, ref, hidden) for each public name a module exports."""
        explicit, globbed = [], []
        for child_id in crate.item(module)["inner"]["module"]["items"]:
            child = crate.item(child_id)
            if child is None or child.get("visibility") != "public":
                continue
            kind = kind_of(child)
            if kind == "impl":
                continue
            hidden = is_hidden(child)
            if kind == "use":
                use = child["inner"]["use"]
                target = None if use["id"] is None else self.resolve(crate, use["id"])
                if target is None:
                    # A primitive, or a name rustdoc could not resolve.
                    if not use["is_glob"]:
                        explicit.append((use["name"], External((use["source"],), "primitive"),
                                         hidden))
                elif use["is_glob"]:
                    globbed += [(n, r, h or hidden) for n, r, h in self.glob(target, globbing)]
                else:
                    explicit.append((use["name"], target, hidden))
            elif kind == "extern_crate":
                inner = child["inner"]["extern_crate"]
                other = self.crate(inner["name"])
                target = (other, other.root) if other else External((inner["name"],), "module")
                explicit.append((inner.get("rename") or inner["name"], target, hidden))
            else:
                explicit.append((child["name"], (crate, int(child_id)), hidden))
        taken = {(name, ns) for name, ref, _ in explicit for ns in self.ns(ref)}
        return explicit + [(name, ref, hidden) for name, ref, hidden in globbed
                           if not any((name, ns) in taken for ns in self.ns(ref))]

    def glob(self, target, globbing: frozenset) -> list:
        if isinstance(target, External):
            return []
        crate, id_ = target
        key = (crate.name, id_)
        if key in globbing:
            return []
        item = crate.item(id_)
        kind = kind_of(item)
        if kind == "module":
            return self.members(crate, id_, globbing | {key})
        if kind == "enum":
            return [(crate.item(v)["name"], (crate, v), False)
                    for v in item["inner"]["enum"]["variants"]]
        return []

    def ns(self, ref) -> set[str]:
        if isinstance(ref, External):
            return namespaces(ref.kind, None)
        crate, id_ = ref
        item = crate.item(id_)
        return namespaces(kind_of(item), item)

    # -- listing ----------------------------------------------------------------

    def emit(self, kind: str, path: list[str], hidden: bool) -> None:
        key = (kind, "::".join(path))
        # A path reachable both ways is hidden only if every way is.
        self.out[key] = self.out.get(key, True) and hidden

    def visit(self, name: str, ref, prefix: list[str], hidden: bool, stack: frozenset) -> None:
        path = prefix + [name]
        if isinstance(ref, External):
            self.emit(ref.kind, path, hidden)
            return
        crate, id_ = ref
        item = crate.item(id_)
        kind = kind_of(item)
        hidden = hidden or is_hidden(item)
        self.emit(kind, path, hidden)
        if kind == "module":
            key = (crate.name, id_)
            if key in stack:
                return
            for child, child_ref, child_hidden in self.members(crate, id_, frozenset()):
                self.visit(child, child_ref, path, hidden or child_hidden, stack | {key})
        elif kind in ("struct", "union", "enum"):
            self.type_members(crate, item, path, hidden)
        elif kind == "trait":
            for trait_item in map(crate.item, item["inner"]["trait"]["items"]):
                if trait_item is not None:
                    self.emit("trait_" + kind_of(trait_item), path + [trait_item["name"]],
                              hidden or is_hidden(trait_item))
        elif kind == "type_alias":
            target = self.alias_target(crate, item)
            if target is not None:
                self.type_members(*target, path, hidden)

    def alias_target(self, crate: Crate, alias: dict) -> tuple[Crate, dict] | None:
        """The struct, union or enum a type alias names, through any chain of
        aliases, if one of the listed crates defines it."""
        seen = set()
        while True:
            named = alias["inner"]["type_alias"]["type"]
            if not isinstance(named, dict) or "resolved_path" not in named:
                return None
            target = self.resolve(crate, named["resolved_path"]["id"])
            if target is None or isinstance(target, External) or target in seen:
                return None
            seen.add(target)
            crate, item = target[0], target[0].item(target[1])
            kind = kind_of(item)
            if kind in ("struct", "union", "enum"):
                return crate, item
            if kind != "type_alias":
                return None
            alias = item

    def type_members(self, crate: Crate, item: dict, path: list[str], hidden: bool) -> None:
        kind = kind_of(item)
        inner = item["inner"][kind]
        fields = []
        if kind == "struct":
            shape = inner["kind"]
            if shape != "unit":
                fields = ([f for f in shape["tuple"] if f is not None] if "tuple" in shape
                          else shape["plain"]["fields"])
        elif kind == "union":
            fields = inner["fields"]
        else:
            for variant in map(crate.item, inner["variants"]):
                variant_path = path + [variant["name"]]
                variant_hidden = hidden or is_hidden(variant)
                self.emit("variant", variant_path, variant_hidden)
                shape = variant["inner"]["variant"]["kind"]
                # A variant's fields are as public as the enum; a tuple variant's
                # are named by their index, as a tuple struct's are.
                variant_fields = []
                if isinstance(shape, dict) and "struct" in shape:
                    variant_fields = shape["struct"]["fields"]
                elif isinstance(shape, dict) and "tuple" in shape:
                    variant_fields = shape["tuple"]
                for field in map(crate.item, variant_fields):
                    if field is not None:
                        self.emit("struct_field", variant_path + [field["name"]],
                                  variant_hidden or is_hidden(field))
        for field in map(crate.item, fields):
            if field is not None and field.get("visibility") == "public":
                self.emit("struct_field", path + [field["name"]], hidden or is_hidden(field))
        for impl in map(crate.item, inner.get("impls", ())):
            if impl is None:
                continue
            body = impl["inner"]["impl"]
            if body["trait"] is not None or body.get("blanket_impl") or body.get("is_synthetic"):
                continue
            impl_hidden = hidden or is_hidden(impl)
            for member in map(crate.item, body["items"]):
                if member is not None and member.get("visibility") == "public":
                    self.emit(kind_of(member), path + [member["name"]],
                              impl_hidden or is_hidden(member))

    def run(self) -> Paths:
        root = self.top.root
        self.emit("module", [self.top.name], is_hidden(self.top.item(root)))
        for name, ref, hidden in self.members(self.top, root, frozenset()):
            self.visit(name, ref, [self.top.name], hidden, frozenset({(self.top.name, root)}))
        return self.out


def walk(directory: Path, libraries: list[str]) -> Paths:
    """Every listed crate's public paths, each crate's JSON read from `directory`."""
    load = directory_loader(directory)
    paths: Paths = {}
    for library in libraries:
        crate = load(library)
        if crate is None:
            raise PublicApiError(f"no rustdoc JSON for {library} in {directory}")
        paths.update(Walker(crate, load).run())
    return paths


# --- the workspace -----------------------------------------------------------


def library_packages(metadata: dict) -> list[dict]:
    """The publishable members with a library target, in the metadata's order."""
    return [p for p in publish_order.publishable(metadata)
            if any(kind in ("lib", "rlib", "dylib", "proc-macro")
                   for target in p.get("targets", []) for kind in target.get("kind", []))]


def library_name(package: dict) -> str:
    for target in package.get("targets", []):
        if any(kind in ("lib", "rlib", "dylib", "proc-macro") for kind in target.get("kind", [])):
            return target["name"].replace("-", "_")
    return package["name"].replace("-", "_")


def features(metadata: dict) -> frozenset[str]:
    """`<package>/<feature>` for each feature of each publishable member, `default`
    and the implicit features of optional dependencies included, as cargo lists them."""
    return frozenset(f"{p['name']}/{feature}" for p in publish_order.publishable(metadata)
                     for feature in p.get("features", {}))


# --- building the JSON -------------------------------------------------------


def _failed(command: list, result: subprocess.CompletedProcess) -> PublicApiError:
    detail = (result.stderr or "").strip() or (result.stdout or "").strip() or "no output"
    return PublicApiError(f"`{' '.join(map(str, command))}` exited {result.returncode}: "
                          f"{detail[-2000:]}")


def _checked(command: list, run: Runner) -> subprocess.CompletedProcess:
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result)
    return result


def toolchain(run: Runner) -> str:
    """`rustc -vV`, which names the toolchain's version and commit."""
    return _checked(["rustc", "-vV"], run).stdout.strip()


def worktree_hash(root: Path, run: Runner) -> str:
    """A hash of every file git tracks or would track in the working tree, as it is
    on disk, so an uncommitted edit is a new tree."""
    listed = _checked(["git", "-C", str(root), "ls-files", "-z", "--cached", "--others",
                       "--exclude-standard"], run).stdout
    digest = hashlib.sha256()
    for name in sorted(set(filter(None, listed.split("\0")))):
        path = root / name
        if not path.is_file():
            continue
        digest.update(name.encode("utf-8") + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def revision_hash(rev: str, run: Runner) -> str:
    return _checked(["git", "rev-parse", "--verify", f"{rev}^{{tree}}"], run).stdout.strip()


def extract(rev: str, tree_hash: str, scratch: Path, run: Runner) -> Path:
    """`rev`'s tree under `<scratch>/baseline/tree/`, extracted again only when it
    holds another tree."""
    directory = scratch / "baseline"
    tree = directory / "tree"
    stamp = directory / "tree.stamp"
    if tree.is_dir() and stamp.is_file() and stamp.read_text().strip() == tree_hash:
        return tree
    # The stamp goes first, so an extraction cut short is never taken for a whole one.
    if stamp.exists():
        stamp.unlink()
    if tree.exists():
        shutil.rmtree(tree)
    tree.mkdir(parents=True)
    archive = directory / "tree.tar"
    _checked(["git", "archive", "--format=tar", f"--output={archive}", rev], run)
    with tarfile.open(archive) as tar:
        try:
            tar.extractall(tree, filter="data")
        except TypeError:  # Python before 3.11.4 has no extraction filters.
            tar.extractall(tree)
    archive.unlink()
    stamp.write_text(tree_hash + "\n")
    return tree


def rustdoc_command(manifest: Path, package: str, target_dir: Path) -> list[str]:
    return ["env", "RUSTC_BOOTSTRAP=1", "cargo", "rustdoc", "--manifest-path", str(manifest),
            "-p", package, "--all-features", "--lib", "--locked",
            "--target-dir", str(target_dir), "--color", "never",
            "--", "-Z", "unstable-options", "--output-format", "json",
            "--document-hidden-items"]


def build(tree: Path, packages: list[dict], json_dir: Path, target_dir: Path, source: str,
          rustc: str, stream: Runner) -> list[str]:
    """Each package's rustdoc JSON in `json_dir`, built unless the stamp there names
    the same source, toolchain, crates and rustdoc command. Returns the library
    names."""
    libraries = [library_name(p) for p in packages]
    command = rustdoc_command(Path("<tree>/Cargo.toml"), "<package>", Path("<target>"))
    stamp = {"source": source, "toolchain": rustc, "crates": libraries, "command": command}
    stamp_path = json_dir / STAMP
    try:
        kept = json.loads(stamp_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        kept = None
    if kept == stamp and all((json_dir / f"{lib}.json").is_file() for lib in libraries):
        print(f"Reusing the rustdoc JSON in {json_dir}", file=sys.stderr)
        return libraries
    if stamp_path.exists():
        stamp_path.unlink()
    json_dir.mkdir(parents=True, exist_ok=True)
    for package, library in zip(packages, libraries):
        print(f"Building rustdoc JSON for {package['name']} in {tree}", file=sys.stderr)
        command = rustdoc_command(tree / "Cargo.toml", package["name"], target_dir)
        result = stream(command)
        if result.returncode != 0:
            raise PublicApiError(f"`{' '.join(command)}` exited {result.returncode}; see its "
                                 f"output above")
        built = target_dir / "doc" / f"{library}.json"
        if not built.is_file():
            raise PublicApiError(f"`cargo rustdoc -p {package['name']}` wrote no {built}")
        shutil.copyfile(built, json_dir / f"{library}.json")
    stamp_path.write_text(json.dumps(stamp, indent=2) + "\n", encoding="utf-8")
    return libraries


def snapshot(rev: str | None, root: Path, scratch: Path, run: Runner,
             stream: Runner) -> Snapshot:
    """The public paths and features at `rev`, or in the working tree at `root`
    when `rev` is None. `scratch` is `target/api-removals/`."""
    rustc = toolchain(run)
    if rev is None:
        tree, source, side = root, worktree_hash(root, run), "head"
    else:
        source = revision_hash(rev, run)
        tree, side = extract(rev, source, scratch, run), "baseline"
    metadata = publish_order.cargo_metadata(tree / "Cargo.toml", run)
    packages = library_packages(metadata)
    libraries = build(tree, packages, scratch / side / "json", scratch / "build", source,
                      rustc, stream)
    return Snapshot(walk(scratch / side / "json", libraries), features(metadata))


# --- the command line --------------------------------------------------------


def _stream(command: list) -> subprocess.CompletedProcess:
    """Run a long command, its output going straight to this process's stderr."""
    try:
        result = subprocess.run(command, cwd=REPO, stdout=sys.stderr, stderr=sys.stderr)
    except OSError as err:
        raise PublicApiError(f"could not run `{command[0]}`: {err}") from err
    return subprocess.CompletedProcess(command, result.returncode, "", "")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="List the workspace's public paths.")
    commands = parser.add_subparsers(dest="command", required=True)
    listing = commands.add_parser("list", help="print `<kind>\\t<path>[\\thidden]` per path")
    listing.add_argument("--rev", metavar="REV",
                         help="a git revision to list instead of the working tree")
    args = parser.parse_args(argv)
    try:
        metadata = publish_order.workspace_metadata()
        scratch = Path(metadata["target_directory"]) / SCRATCH
        found = snapshot(args.rev, REPO, scratch, publish_order.run_command, _stream)
    except (PublicApiError, publish_order.PublishOrderError) as err:
        print(f"::error::{err}", file=sys.stderr)
        return 2
    for (kind, path), hidden in sorted(found.paths.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        print("\t".join([kind, path, *(["hidden"] if hidden else [])]))
    for feature in sorted(found.features):
        print(f"feature\t{feature}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
