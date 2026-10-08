#!/usr/bin/env python3
"""The generated regions of the docs and the nightly workflows, from Cargo metadata.

Each crate declares its facts once, in its own `[package.metadata.polyoxide]`,
and each mirror directory under `docs/specs/` once, in the root manifest's
`[workspace.metadata.polyoxide.mirrors]`. This script is the only writer of the
lines between a `generated:begin <id>` marker and its `generated:end <id>`,
written in the host file's comment leader: `<!-- generated:begin <id> -->` in
Markdown, `# generated:begin <id>` in YAML and TOML. Text outside the markers is
never touched, and each generated line takes its begin marker's indentation.

    --write  Rewrite every region.
    --check  Print a unified diff and exit 1 when a committed region differs from
             what the metadata produces. CI runs it through
             `.github/scripts/tests/test_gen_registry.py`.

Exit codes: 0 when the regions match or were written, 1 when they differ or the
metadata is refused, 2 for bad arguments (argparse's own).

The crate table:

    [package.metadata.polyoxide]
    readme = "..."            # README.md's line for the crate
    venue = "polymarket"      # with `products`; both omitted for no venue
    products = ["clob"]       # each (venue, product) pair is the workspace's only one
    mirrors = ["clob"]        # docs/specs/ directories the crate covers
    notes = ["..."]           # annotations for CLAUDE.md's crate list that
                              # nothing else can derive

    [package.metadata.polyoxide.live.<test target>]
    suite = "live"            # one nightly job per (crate, suite)
    timeout = 15              # minutes; a job takes its targets' maximum
    features = ["ws"]         # the target's required-features
    secrets = ["NAME"]        # every env name the target reads, exactly
    note = "..."              # optional, for SELF-HEALING.md's table

The workspace table:

    [workspace.metadata.polyoxide]
    stage = "S1"              # the release stage (AD-16) docs/ARCHITECTURE.md states

docs/ARCHITECTURE.md also holds the architecture spine's guide, `GUIDE`, copied
byte for byte into its `architecture-guide` region, so regenerating the guide from
the spine and running `--write` is the whole update.

The mirror table, in the order every generated list follows:

    [workspace.metadata.polyoxide.mirrors.<directory>]
    name, section, base_urls, description, crate_note?, exclude?
    [[workspace.metadata.polyoxide.mirrors.<directory>.specs]]
    id, kind, vendored, url?, covers?, note?, exclude?

`section` is `covered`, `not-implemented`, `other-venue` or `excluded` (prose of
its own in INDEX.md). `exclude`, on a directory or a spec, keeps it out of
nightly-schema.yml and is the one sentence every exclusion list gives as the
reason. A spec's `covers` is its INDEX.md AsyncAPI row; its `note` follows its
URL in INDEX.md's list.

Stdlib only. `tomllib` reads the manifests, since it keeps `[features]` in the
order written, and `cargo metadata`, through scripts/publish_order.py, gives the
dependencies, the test targets and the publish order.

Usage:
    python3 scripts/gen_registry.py --check
    python3 scripts/gen_registry.py --write
"""
from __future__ import annotations

import argparse
import difflib
import re
import sys
import textwrap
import tomllib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish_order  # noqa: E402

REPO = publish_order.REPO
SPECS = "docs/specs"
GUIDE = ("_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/"
         "ARCHITECTURE-GUIDE.md")
# The guide's heading that says what each stage is, as GitHub anchors it.
STAGE_ANCHOR = "which-stage-the-workspace-is-in-ad-16"
UMBRELLA = "polyoxide"
CLI = "polyoxide-cli"

ID = re.compile(r"^[a-z][a-z0-9-]*$")
SECRET = re.compile(r"^[A-Z][A-Z0-9_]*$")
STAGE = re.compile(r"^S[1-9][0-9]*$")
SECTIONS = ("covered", "not-implemented", "other-venue", "excluded")
KINDS = {"openapi": "OpenAPI", "asyncapi": "AsyncAPI"}
# The page an INDEX.md table links for a mirror: the first of these it has.
PAGES = ("INDEX.md", "README.md", "OBSERVED.md")
NUMBERS = ("zero one two three four five six seven eight nine ten eleven twelve "
           "thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty").split()

WORKSPACE_KEYS = {"stage", "mirrors"}
CRATE_KEYS = {"readme", "venue", "products", "mirrors", "notes", "live"}
LIVE_KEYS = {"suite", "timeout", "features", "secrets", "note"}
MIRROR_KEYS = {"name", "section", "base_urls", "description", "crate_note", "exclude", "specs"}
SPEC_KEYS = {"id", "kind", "url", "vendored", "covers", "note", "exclude"}


class RegistryError(Exception):
    """Metadata or a marker the generator refuses. The message names where."""


@dataclass(frozen=True)
class Spec:
    id: str
    kind: str
    vendored: str
    url: str | None = None
    covers: str | None = None
    note: str | None = None
    exclude: str | None = None


@dataclass(frozen=True)
class Mirror:
    dir: str
    name: str
    section: str
    base_urls: tuple[str, ...]
    description: str
    # The page INDEX.md links, relative to docs/specs/.
    page: str
    specs: tuple[Spec, ...] = ()
    # The crates whose `mirrors` name this directory.
    crates: tuple[str, ...] = ()
    crate_note: str | None = None
    exclude: str | None = None


@dataclass(frozen=True)
class Live:
    target: str
    suite: str
    timeout: int
    features: tuple[str, ...] = ()
    secrets: tuple[str, ...] = ()
    note: str | None = None


@dataclass(frozen=True)
class Dependency:
    """A normal or build dependency on another workspace member."""

    name: str
    features: tuple[str, ...] = ()
    default_features: bool = True
    # The features that turn on an optional dependency. Empty when it is required.
    enabled_by: tuple[str, ...] = ()
    # Whether one of those features is on by default.
    default_on: bool = False


@dataclass(frozen=True)
class Crate:
    name: str
    # The crate's directory, relative to the repository.
    dir: str
    readme: str
    published: bool
    venue: str | None = None
    products: tuple[str, ...] = ()
    mirrors: tuple[str, ...] = ()
    notes: tuple[str, ...] = ()
    live: tuple[Live, ...] = ()
    needs: tuple[Dependency, ...] = ()
    # Versioned dev-dependencies: not needed to build, but published first.
    published_after: tuple[str, ...] = ()
    # `[features]` as written, `default` included.
    features: tuple[tuple[str, tuple[str, ...]], ...] = ()


@dataclass(frozen=True)
class Job:
    """One nightly-behavioral job: a crate's live targets that share a suite."""

    crate: str
    suite: str
    targets: tuple[Live, ...]

    @property
    def id(self) -> str:
        return f"live-{self.crate}-{self.suite}"

    @property
    def timeout(self) -> int:
        return max(t.timeout for t in self.targets)

    @property
    def features(self) -> tuple[str, ...]:
        return tuple(sorted({f for t in self.targets for f in t.features}))

    @property
    def secrets(self) -> tuple[str, ...]:
        return tuple(sorted({s for t in self.targets for s in t.secrets}))

    @property
    def flags(self) -> str:
        """`--features <union>` first, then `--test <target>` in name order."""
        flags = [f"--features {','.join(self.features)}"] if self.features else []
        flags += [f"--test {t.target}" for t in sorted(self.targets, key=lambda t: t.target)]
        return " ".join(flags)


@dataclass(frozen=True)
class Exclusion:
    label: str
    path: str
    reason: str


@dataclass(frozen=True)
class Registry:
    # In publish order, then the unpublished members by name.
    crates: tuple[Crate, ...]
    # As the root manifest lists them.
    mirrors: tuple[Mirror, ...]
    # The release stage, `S1` and on.
    stage: str
    # The architecture guide's lines, without their line endings.
    guide: tuple[str, ...] = ()

    def crate(self, name: str) -> Crate:
        return next(c for c in self.crates if c.name == name)

    @property
    def published(self) -> list[Crate]:
        return [c for c in self.crates if c.published]

    def jobs(self) -> list[Job]:
        jobs = []
        for crate in self.crates:
            suites: dict[str, list[Live]] = {}
            for live in crate.live:
                suites.setdefault(live.suite, []).append(live)
            jobs += [Job(crate.name, suite, tuple(targets)) for suite, targets in suites.items()]
        return jobs

    def watched(self) -> list[tuple[Mirror, Spec]]:
        """The specs nightly-schema.yml diffs: every OpenAPI one, then every AsyncAPI one."""
        return [(m, s) for kind in KINDS for m in self.mirrors if not m.exclude
                for s in m.specs if s.kind == kind and not s.exclude]

    def exclusions(self) -> list[Exclusion]:
        out = []
        for m in self.mirrors:
            if m.exclude:
                out.append(Exclusion(m.name, f"{SPECS}/{m.dir}/", m.exclude))
            else:
                out += [Exclusion(s.id, s.vendored, s.exclude) for s in m.specs if s.exclude]
        return out


# --- loading ---------------------------------------------------------------


def _toml(path: Path) -> dict:
    with path.open("rb") as file:
        return tomllib.load(file)


def load(root: Path = REPO, metadata: dict | None = None,
         run: publish_order.Runner = publish_order.run_command) -> Registry:
    """The registry for the workspace at `root`, from its manifests and `cargo metadata`."""
    if metadata is None:
        metadata = publish_order.cargo_metadata(root / "Cargo.toml", run)
    manifests = {p["name"]: _toml(Path(p["manifest_path"]))
                 for p in publish_order.members(metadata)}
    return build(root, metadata, _toml(root / "Cargo.toml"), manifests)


def build(root: Path, metadata: dict, workspace: dict, manifests: dict[str, dict]) -> Registry:
    """Check the metadata and assemble the registry. `manifests` maps a member to its TOML."""
    packages = {p["name"]: p for p in publish_order.members(metadata)}
    order = [p["name"] for p in publish_order.publish_order(metadata)]
    order += sorted(n for n, p in packages.items() if not publish_order.is_publishable(p))
    crates = tuple(_crate(root, packages[n], manifests[n], set(packages)) for n in order)
    _check_products(crates)
    return Registry(crates, _mirrors(root, workspace, crates), _stage(workspace), _guide(root))


def _get(table: dict, *keys: str) -> dict:
    for key in keys:
        table = table.get(key, {})
    return table


def _unknown(where: str, table: dict, allowed: set[str]) -> None:
    extra = sorted(set(table) - allowed)
    if extra:
        raise RegistryError(f"{where}: unknown key{'s' if len(extra) > 1 else ''} "
                            f"{', '.join(extra)}; expected some of {', '.join(sorted(allowed))}")


def _string(where: str, value, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str) or not value.strip():
        raise RegistryError(f"{where} must be a non-empty string, not {value!r}")
    return value


def _strings(where: str, value) -> tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, list) or not all(isinstance(v, str) and v for v in value):
        raise RegistryError(f"{where} must be a list of non-empty strings, not {value!r}")
    if len(set(value)) != len(value):
        raise RegistryError(f"{where} lists a value twice: {value!r}")
    return tuple(value)


def _id(where: str, value) -> str:
    if not isinstance(value, str) or not ID.match(value):
        raise RegistryError(f"{where} {value!r} is not an id: ids match {ID.pattern}")
    return value


def _crate(root: Path, package: dict, manifest: dict, workspace: set[str]) -> Crate:
    name = package["name"]
    where = f"{name}: [package.metadata.polyoxide]"
    meta = _get(manifest, "package", "metadata", "polyoxide")
    if not meta:
        raise RegistryError(f"{where} is missing; every member registers itself there")
    _unknown(where, meta, CRATE_KEYS)
    venue = meta.get("venue")
    products = _strings(f"{where} products", meta.get("products"))
    if (venue is None) != (not products):
        raise RegistryError(f"{where} declares {'products' if products else 'a venue'} "
                            f"without {'a venue' if products else 'products'}; give both or neither")
    if venue is not None:
        _id(f"{where} venue", venue)
    for product in products:
        _id(f"{where} product", product)
    mirrors = _strings(f"{where} mirrors", meta.get("mirrors"))
    live = tuple(_live(f"{name}: [package.metadata.polyoxide.live.{target}]", target, entry)
                 for target, entry in _live_table(where, meta).items())
    features = tuple((k, tuple(v)) for k, v in manifest.get("features", {}).items())
    directory = Path(package["manifest_path"]).resolve().parent
    return Crate(
        name=name,
        dir=directory.relative_to(root.resolve()).as_posix(),
        readme=_string(f"{where} readme", meta.get("readme")),
        published=publish_order.is_publishable(package),
        venue=venue,
        products=products,
        mirrors=mirrors,
        notes=_strings(f"{where} notes", meta.get("notes")),
        live=live,
        needs=_needs(package, workspace, dict(features)),
        published_after=_published_after(package, workspace),
        features=features,
    )


def _live_table(where: str, meta: dict) -> dict:
    live = meta.get("live", {})
    if not isinstance(live, dict) or not all(isinstance(v, dict) for v in live.values()):
        raise RegistryError(f"{where} live must hold one table per test target")
    return live


def _live(where: str, target: str, entry: dict) -> Live:
    _unknown(where, entry, LIVE_KEYS)
    missing = sorted({"suite", "timeout", "features", "secrets"} - set(entry))
    if missing:
        raise RegistryError(f"{where} lacks {', '.join(missing)}")
    timeout = entry["timeout"]
    if not isinstance(timeout, int) or isinstance(timeout, bool) or timeout <= 0:
        raise RegistryError(f"{where} timeout must be a positive number of minutes, not {timeout!r}")
    secrets = _strings(f"{where} secrets", entry["secrets"])
    for secret in secrets:
        if not SECRET.match(secret) or secret.startswith("GITHUB_"):
            raise RegistryError(f"{where} secret {secret!r} is not a repository secret name")
    note = _string(f"{where} note", entry.get("note"), optional=True)
    # A note that states the budget must state this one.
    for minutes in re.findall(r"(\d+)-minute", note or ""):
        if int(minutes) != timeout:
            raise RegistryError(f"{where} note says {minutes} minutes, but timeout is {timeout}")
    return Live(target, _id(f"{where} suite", entry["suite"]), timeout,
                _strings(f"{where} features", entry["features"]), secrets, note)


def _default_closure(features: dict[str, tuple[str, ...]]) -> set[str]:
    on, todo = set(), list(features.get("default", ()))
    while todo:
        feature = todo.pop()
        if feature not in on and feature in features:
            on.add(feature)
            todo += [f for f in features[feature] if ":" not in f and "/" not in f]
    return on


def _needs(package: dict, workspace: set[str], features: dict) -> tuple[Dependency, ...]:
    """The members `package` needs to build: its normal and build dependencies."""
    defaults = _default_closure(features)
    needs = {}
    for dep in package["dependencies"]:
        if dep.get("path") is None or dep["name"] not in workspace or dep["kind"] == "dev":
            continue
        name = dep["name"]
        enabled_by = ()
        if dep.get("optional"):
            enabled_by = tuple(f for f, items in features.items() if f"dep:{name}" in items)
            # Without `dep:`, cargo makes an implicit feature named after the dependency.
            enabled_by = enabled_by or (name,)
        needs[name] = Dependency(
            name=name,
            features=tuple(dep.get("features") or ()),
            default_features=dep.get("uses_default_features", True),
            enabled_by=enabled_by,
            default_on=any(f in defaults for f in enabled_by),
        )
    return tuple(needs[n] for n in sorted(needs))


def _published_after(package: dict, workspace: set[str]) -> tuple[str, ...]:
    built = {d["name"] for d in package["dependencies"] if d["kind"] != "dev"}
    return tuple(sorted({
        d["name"] for d in package["dependencies"]
        if d["kind"] == "dev" and d.get("path") is not None and d["name"] in workspace
        and not publish_order.is_path_only(d) and d["name"] not in built
    }))


def _check_products(crates: tuple[Crate, ...]) -> None:
    owners: dict[tuple[str, str], list[str]] = {}
    for crate in crates:
        for product in crate.products:
            owners.setdefault((crate.venue, product), []).append(crate.name)
    for (venue, product), names in owners.items():
        if len(names) > 1:
            raise RegistryError(f"({venue}, {product}) is declared by more than one crate: "
                                f"{', '.join(names)}. Each (venue, product) pair has one crate.")


def _stage(workspace: dict) -> str:
    where = "[workspace.metadata.polyoxide]"
    meta = _get(workspace, "workspace", "metadata", "polyoxide")
    _unknown(where, meta, WORKSPACE_KEYS)
    stage = meta.get("stage")
    if not isinstance(stage, str) or not STAGE.fullmatch(stage):
        raise RegistryError(f"{where} stage {stage!r} is not a release stage: stages match "
                            f"{STAGE.pattern}, as in `stage = \"S1\"`")
    return stage


def _guide(root: Path) -> tuple[str, ...]:
    try:
        text = (root / GUIDE).read_bytes().decode("utf-8")
    except (OSError, UnicodeDecodeError) as err:
        raise RegistryError(f"{GUIDE} cannot be read: {err}") from err
    if not text.endswith("\n"):
        raise RegistryError(f"{GUIDE} must end with a newline, or docs/ARCHITECTURE.md "
                            f"cannot hold it byte for byte")
    return tuple(text[:-1].split("\n"))


def _mirrors(root: Path, workspace: dict, crates: tuple[Crate, ...]) -> tuple[Mirror, ...]:
    table = _get(workspace, "workspace", "metadata", "polyoxide", "mirrors")
    for crate in crates:
        for directory in crate.mirrors:
            if directory not in table:
                raise RegistryError(
                    f"{crate.name} lists mirror {directory!r}, which "
                    f"[workspace.metadata.polyoxide.mirrors] does not declare")
    specs_dir = root / SPECS
    undeclared = sorted(p.name for p in specs_dir.iterdir() if p.is_dir() and p.name not in table)
    if undeclared:
        raise RegistryError(f"{SPECS}/ has directories no mirror entry declares: "
                            f"{', '.join(undeclared)}. Add a [workspace.metadata.polyoxide."
                            f"mirrors.<directory>] table for each.")
    mirrors, ids = [], set()
    for directory, entry in table.items():
        mirror = _mirror(root, directory, entry,
                         tuple(c.name for c in crates if directory in c.mirrors))
        for spec in mirror.specs:
            if spec.id in ids:
                raise RegistryError(f"spec id {spec.id!r} is declared twice; it names a "
                                    f"`spec:{spec.id}` label, so it must be unique")
            ids.add(spec.id)
        mirrors.append(mirror)
    return tuple(mirrors)


def _mirror(root: Path, directory: str, entry: dict, crates: tuple[str, ...]) -> Mirror:
    where = f"[workspace.metadata.polyoxide.mirrors.{directory}]"
    _id(f"{where} directory", directory)
    _unknown(where, entry, MIRROR_KEYS)
    path = root / SPECS / directory
    if not path.is_dir():
        raise RegistryError(f"{where}: {SPECS}/{directory}/ does not exist")
    page = next((p for p in PAGES if (path / p).is_file()), None)
    if page is None:
        raise RegistryError(f"{where}: {SPECS}/{directory}/ has none of {', '.join(PAGES)} to link")
    section = entry.get("section")
    if section not in SECTIONS:
        raise RegistryError(f"{where} section {section!r} is not one of {', '.join(SECTIONS)}")
    exclude = _string(f"{where} exclude", entry.get("exclude"), optional=True)
    if section == "excluded" and not exclude:
        raise RegistryError(f"{where} is in section `excluded` without an `exclude` reason")
    if section in ("covered", "other-venue") and not crates:
        raise RegistryError(f"{where} is in section `{section}`, but no crate lists it in mirrors")
    if section == "not-implemented" and crates:
        raise RegistryError(f"{where} is `not-implemented`, but {', '.join(crates)} list it")
    base_urls = _strings(f"{where} base_urls", entry.get("base_urls"))
    if not base_urls:
        raise RegistryError(f"{where} needs at least one base URL")
    specs = entry.get("specs", [])
    if not isinstance(specs, list):
        raise RegistryError(f"{where} specs must be an array of tables")
    return Mirror(
        dir=directory,
        name=_string(f"{where} name", entry.get("name")),
        section=section,
        base_urls=base_urls,
        description=_string(f"{where} description", entry.get("description")),
        page=f"{directory}/{page}",
        specs=tuple(_spec(root, directory, where, s, bool(exclude)) for s in specs),
        crates=crates,
        crate_note=_string(f"{where} crate_note", entry.get("crate_note"), optional=True),
        exclude=exclude,
    )


def _spec(root: Path, directory: str, mirror_where: str, entry: dict, excluded: bool) -> Spec:
    if not isinstance(entry, dict):
        raise RegistryError(f"{mirror_where} specs must be an array of tables")
    spec_id = _id(f"{mirror_where} spec id", entry.get("id"))
    where = f"{mirror_where} spec {spec_id!r}"
    _unknown(where, entry, SPEC_KEYS)
    kind = entry.get("kind")
    if kind not in KINDS:
        raise RegistryError(f"{where} kind {kind!r} is not one of {', '.join(KINDS)}")
    vendored = _string(f"{where} vendored", entry.get("vendored"))
    if not vendored.startswith(f"{SPECS}/{directory}/") or not (root / vendored).is_file():
        raise RegistryError(f"{where} vendored {vendored!r} is not a file in {SPECS}/{directory}/")
    spec = Spec(
        id=spec_id,
        kind=kind,
        vendored=vendored,
        url=_string(f"{where} url", entry.get("url"), optional=True),
        covers=_string(f"{where} covers", entry.get("covers"), optional=True),
        note=_string(f"{where} note", entry.get("note"), optional=True),
        exclude=_string(f"{where} exclude", entry.get("exclude"), optional=True),
    )
    if spec.url is None and not (excluded or spec.exclude):
        raise RegistryError(f"{where} has no url, so nightly-schema.yml cannot fetch it; "
                            f"give one or an `exclude` reason")
    if kind == "asyncapi" and not spec.covers:
        raise RegistryError(f"{where} is AsyncAPI, so INDEX.md's table needs its `covers`")
    return spec


# --- the secrets scan ------------------------------------------------------

# A string literal in the position that names an environment variable.
ENV_LITERAL = re.compile(
    r"(?<![A-Za-z0-9_])(?:var|var_os|env!|option_env!)\s*\(\s*\"([A-Za-z_][A-Za-z0-9_]*)\"")
# Library calls that read environment variables the calling file never names,
# and the constants module in the library listing what they read.
LOADERS = {
    "Account::from_env(": ("polyoxide-clob/src/account/mod.rs", "env"),
}
CONSTANT = re.compile(r"const\s+[A-Z0-9_]+\s*:\s*&str\s*=\s*\"([A-Za-z_][A-Za-z0-9_]*)\"")


def _code(source: str) -> str:
    """`source` without its `//` comment lines, so prose naming a variable is not a read."""
    return "\n".join(line for line in source.splitlines() if not line.lstrip().startswith("//"))


def loader_names(path: Path, module: str) -> set[str]:
    """The string constants in `pub mod <module> { ... }` of the Rust file at `path`."""
    source = path.read_text()
    match = re.search(rf"^(\s*)pub mod {module} \{{\n(.*?)^\1\}}", source, re.M | re.S)
    if match is None:
        raise RegistryError(f"{path} has no `pub mod {module}` block to read env names from")
    return set(CONSTANT.findall(match.group(2)))


# A `mod x;` declaration, whose body is another file.
MODULE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;", re.M)


def module_files(path: Path) -> list[Path]:
    """A test target's file, then each `mod x;` it declares, as `x.rs` or `x/mod.rs` beside it.

    One level: a target's code is its own file plus shared modules such as
    `tests/common/mod.rs`.
    """
    files = [path]
    for name in MODULE.findall(_code(path.read_text())):
        for candidate in (path.parent / f"{name}.rs", path.parent / name / "mod.rs"):
            if candidate.is_file():
                files.append(candidate)
                break
    return files


def target_source(path: Path) -> str:
    """The text of a test target and the modules it declares, for `env_names`."""
    return "\n".join(file.read_text() for file in module_files(path))


def env_names(source: str, root: Path = REPO) -> set[str]:
    """The environment variables a live test target reads.

    A static scan, until the credential loaders of Epic 2 replace it: string
    literals passed to `var`, `var_os`, `env!` or `option_env!`, plus what each
    library loader in `LOADERS` reads, from its constants. Pass `target_source`,
    so a read in a shared module counts.
    """
    code = _code(source)
    names = set(ENV_LITERAL.findall(code))
    for call, (path, module) in LOADERS.items():
        if call in code:
            names |= loader_names(root / path, module)
    return names


# --- rendering -------------------------------------------------------------


def _number(n: int) -> str:
    return NUMBERS[n] if n < len(NUMBERS) else str(n)


def _code_list(names) -> str:
    return ", ".join(f"`{n}`" for n in names)


def _and(items: list[str]) -> str:
    return items[0] if len(items) == 1 else f"{', '.join(items[:-1])} and {items[-1]}"


def _wrap(text: str, width: int, first: str, rest: str) -> list[str]:
    return textwrap.wrap(text, width=width, initial_indent=first, subsequent_indent=rest,
                         break_long_words=False, break_on_hyphens=False)


def _dependency(dep: Dependency) -> str:
    parts = []
    if dep.features:
        parts.append(f"with {_code_list(dep.features)}")
    if not dep.default_features:
        parts.append("without default features")
    if dep.enabled_by:
        parts.append(f"under {_code_list(dep.enabled_by)}" + (", on by default" if dep.default_on else ""))
    return f"`{dep.name}`" + (f" ({', '.join(parts)})" if parts else "")


def _crate_cell(mirror: Mirror) -> str:
    cell = _code_list(mirror.crates) or "—"
    return f"{cell} ({mirror.crate_note})" if mirror.crate_note else cell


def readme_crates(r: Registry) -> list[str]:
    lines = ["| Crate | Description |", "|-------|-------------|"]
    return lines + [f"| [{c.name}](./{c.dir}) | {c.readme} |"
                    for c in sorted(r.crates, key=lambda c: c.name)]


def index_upstream(r: Registry) -> list[str]:
    return [f"- {m.name}: {s.url}" + (f" ({s.note})" if s.note else "")
            for m in r.mirrors for s in m.specs if s.kind == "openapi" and s.url]


def _index_table(r: Registry, section: str, crate: bool) -> list[str]:
    lines = ["| API | Base URL | Description |" + (" Crate |" if crate else ""),
             "|-----|----------|-------------|" + ("-------|" if crate else "")]
    for m in r.mirrors:
        if m.section == section:
            row = [f"[{m.name}]({m.page})", _code_list(m.base_urls), m.description]
            lines.append("| " + " | ".join(row + ([_crate_cell(m)] if crate else [])) + " |")
    return lines


def index_covered(r: Registry) -> list[str]:
    return _index_table(r, "covered", crate=True)


def index_not_implemented(r: Registry) -> list[str]:
    return _index_table(r, "not-implemented", crate=False)


def index_other_venues(r: Registry) -> list[str]:
    return _index_table(r, "other-venue", crate=True)


def index_asyncapi(r: Registry) -> list[str]:
    lines = ["| Spec | Covers | Crate |", "|------|--------|-------|"]
    for m in r.mirrors:
        for s in m.specs:
            if s.kind == "asyncapi":
                page = s.vendored.removeprefix(f"{SPECS}/")
                lines.append(f"| [{page}]({page}) | {s.covers} | {_code_list(m.crates) or '—'} |")
    return lines


def claude_crate_count(r: Registry) -> list[str]:
    sentence = (f"{_number(len(r.crates)).capitalize()} crates, in publish order, each with the "
                f"workspace crates its build needs")
    if len(r.published) < len(r.crates):
        sentence += ". Crates that are not published come last"
    return [sentence + ":"]


def claude_graph(r: Registry) -> list[str]:
    lines = []
    for c in r.crates:
        needs = ", ".join(_dependency(d) for d in c.needs) or "nothing in the workspace"
        parts = [f"`{c.name}` — {c.readme}", f"needs: {needs}"]
        if c.published_after:
            parts.append(f"published after {_code_list(c.published_after)}, "
                         f"a versioned dev-dependency")
        lines.append("- " + "; ".join(parts + list(c.notes)))
    return lines


def claude_umbrella_features(r: Registry) -> list[str]:
    features = dict(r.crate(UMBRELLA).features)
    named = [f for f in features if f != "default"]
    shown = []
    for name in named:
        items = features[name]
        if name == "full":
            rest = [f for f in named if f != "full" and f not in items]
            note = f"all but {_code_list(rest)}" if rest else "all"
        elif any(i.startswith("dep:") for i in items):
            note = None
        else:
            note = _code_list(i for i in items if "/" in i) or None
        shown.append(f"`{name}`" + (f" ({note})" if note else ""))
    default = " + ".join(features.get("default", ())) or "none"
    return [f"**{UMBRELLA}** (the unified crate) uses feature flags: {', '.join(shown)}. "
            f"Default = {default}."]


def claude_cli_deps(r: Registry) -> list[str]:
    cli = r.crate(CLI)
    names = {d.name for d in cli.needs}
    if UMBRELLA in names:
        opening = f"Note: `{CLI}` depends on the unified `{UMBRELLA}` crate."
    else:
        opening = f"Note: `{CLI}` does **not** depend on the unified `{UMBRELLA}` crate."
    direct = [_dependency(d) for d in cli.needs if not d.enabled_by and d.name != UMBRELLA]
    sentence = f"{opening} It depends directly on the component crates — {_and(direct)} —"
    groups: dict[tuple[str, ...], list[str]] = {}
    for d in cli.needs:
        if d.enabled_by:
            groups.setdefault(d.enabled_by, []).append(_dependency(Dependency(d.name, d.features)))
    extras = [f"{_and(deps)} only under the optional {_code_list(by)} feature"
              f"{'s' if len(by) > 1 else ''}" for by, deps in groups.items()]
    return [f"{sentence} plus {'; '.join(extras)}." if extras else sentence.removesuffix(" —") + "."]


def claude_publish_order(r: Registry) -> list[str]:
    return [f"Today's order: {' → '.join(f'`{c.name}`' for c in r.published)}."]


def claude_nightly(r: Registry) -> list[str]:
    lines = []
    for job in r.jobs():
        targets = ", ".join(
            f"`{t.target}`" + (f" with `--features {','.join(t.features)}`" if t.features else "")
            for t in sorted(job.targets, key=lambda t: t.target))
        line = f"- `{job.id}` ({job.timeout} min): {targets}"
        lines.append(line + (f"; secrets {_code_list(job.secrets)}" if job.secrets else ""))
    return lines


def claude_schema_watch(r: Registry) -> list[str]:
    groups = []
    for kind, label in KINDS.items():
        ids = [s.id for _, s in r.watched() if s.kind == kind]
        if ids:
            groups.append(f"{_number(len(ids))} {label} ({_code_list(ids)})")
    return [f"It watches {_and(groups)}, each filed under its own `spec:<id>` label."]


def claude_schema_exclusions(r: Registry) -> list[str]:
    return [f"- {e.label} (`{e.path}`): {e.reason}" for e in r.exclusions()]


def selfheal_behavioral(r: Registry) -> list[str]:
    lines = ["| Crate | Test binaries |", "|-------|---------------|"]
    for crate in r.crates:
        jobs = [j for j in r.jobs() if j.crate == crate.name]
        if not jobs:
            continue
        suites = []
        for job in jobs:
            targets = []
            for t in sorted(job.targets, key=lambda t: t.target):
                parts = [f"built with `--features {','.join(t.features)}`"] if t.features else []
                parts += [t.note] if t.note else []
                targets.append(f"`{t.target}`" + (f" ({'; '.join(parts)})" if parts else ""))
            prefix = f"`{job.suite}`: " if len(jobs) > 1 else ""
            suites.append(prefix + ", ".join(targets))
        lines.append(f"| {crate.name} | {'; '.join(suites)} |")
    return lines


def selfheal_watch(r: Registry) -> list[str]:
    lines = ["| Entry | Upstream | Vendored mirror |", "|-------|----------|-----------------|"]
    return lines + [f"| {s.id} | `{s.url.split('://', 1)[-1]}`"
                    + (f" ({s.note})" if s.note else "") + f" | `{s.vendored}` |"
                    for _, s in r.watched()]


def selfheal_exclusions(r: Registry) -> list[str]:
    lines = []
    for e in r.exclusions():
        lines += _wrap(f"**{e.label}** (`{e.path}`) — {e.reason}", 80, "- ", "  ")
    return lines


def nightly_live_jobs(r: Registry) -> list[str]:
    lines = []
    for job in r.jobs():
        if lines:
            lines.append("")
        lines += [
            f"{job.id}:",
            f"  name: Live tests ({job.crate}, {job.suite})",
            "  runs-on: ubuntu-latest",
            f"  timeout-minutes: {job.timeout}",
        ]
        if job.secrets:
            lines.append("  env:")
            lines += [f"    {s}: ${{{{ secrets.{s} }}}}" for s in job.secrets]
        lines += [
            "  steps:",
            "    - uses: actions/checkout@v5",
            "    - uses: ./.github/actions/live-suite",
            "      with:",
            f"        crate: {job.crate}",
            f"        suite: {job.suite}",
            f'        flags: "{job.flags}"',
        ]
    return lines


def nightly_aggregate_needs(r: Registry) -> list[str]:
    return ["needs:"] + [f"  - {job.id}" for job in r.jobs()]


def schema_watch(r: Registry) -> list[str]:
    """The `include:` rows, their columns aligned."""
    rows = [(f"id: {s.id},", f'url: "{s.url}",', s.vendored) for _, s in r.watched()]
    id_width = max(len(i) for i, _, _ in rows) + 1
    url_width = max(len(u) for _, u, _ in rows) + 1
    return [f"- {{ {i.ljust(id_width)}{u.ljust(url_width)}vendored: {v} }}" for i, u, v in rows]


def schema_exclusions(r: Registry) -> list[str]:
    lines = []
    for e in r.exclusions():
        lines += _wrap(f"{e.label} ({e.path}): {e.reason}", 72, "#   - ", "#     ")
    return lines


def architecture_stage(r: Registry) -> list[str]:
    return [f"**The workspace is in stage {r.stage}.** [Which stage the workspace is in]"
            f"(#{STAGE_ANCHOR}) says what each stage changes."]


def architecture_guide(r: Registry) -> list[str]:
    return list(r.guide)


Renderer = Callable[[Registry], list[str]]

# Each generated file, its marker style, and the regions it must hold.
FILES: dict[str, dict[str, Renderer]] = {
    "README.md": {"readme-crates": readme_crates},
    "docs/specs/INDEX.md": {
        "index-upstream": index_upstream,
        "index-covered": index_covered,
        "index-not-implemented": index_not_implemented,
        "index-other-venues": index_other_venues,
        "index-asyncapi": index_asyncapi,
    },
    "CLAUDE.md": {
        "claude-crate-count": claude_crate_count,
        "claude-graph": claude_graph,
        "claude-umbrella-features": claude_umbrella_features,
        "claude-cli-deps": claude_cli_deps,
        "claude-publish-order": claude_publish_order,
        "claude-nightly": claude_nightly,
        "claude-schema-watch": claude_schema_watch,
        "claude-schema-exclusions": claude_schema_exclusions,
    },
    "SELF-HEALING.md": {
        "selfheal-behavioral": selfheal_behavioral,
        "selfheal-watch": selfheal_watch,
        "selfheal-exclusions": selfheal_exclusions,
    },
    ".github/workflows/nightly-behavioral.yml": {
        "nightly-live-jobs": nightly_live_jobs,
        "nightly-aggregate-needs": nightly_aggregate_needs,
    },
    ".github/workflows/nightly-schema.yml": {
        "schema-watch": schema_watch,
        "schema-exclusions": schema_exclusions,
    },
    "docs/ARCHITECTURE.md": {
        "architecture-stage": architecture_stage,
        "architecture-guide": architecture_guide,
    },
}


# --- splicing --------------------------------------------------------------

MARKERS = {
    "markdown": re.compile(
        r"^(?P<indent>[ \t]*)<!-- generated:(?P<edge>begin|end) (?P<id>\S+) -->[ \t]*$"),
    "hash": re.compile(r"^(?P<indent>[ \t]*)# generated:(?P<edge>begin|end) (?P<id>\S+)[ \t]*$"),
}


def marker_style(path: str) -> str:
    return "markdown" if path.endswith(".md") else "hash"


def _lines(text: str) -> list[str]:
    """`text` split after each `\\n`, every line keeping its ending."""
    return re.findall(r"[^\n]*\n|[^\n]+\Z", text)


def splice(text: str, style: str, regions: dict[str, list[str]], label: str = "<text>") -> str:
    """`text` with each region's lines replaced, indented as its begin marker is.

    Every region in `regions` must appear exactly once, and no other may.
    """
    pattern = MARKERS[style]
    out, seen, current = [], set(), None
    for number, line in enumerate(_lines(text), start=1):
        body = line.rstrip("\r\n")
        match = pattern.match(body)
        where = f"{label}:{number}"
        if current is None:
            out.append(line)
            if match is None:
                continue
            region = match["id"]
            if match["edge"] == "end":
                raise RegistryError(f"{where}: `generated:end {region}` has no begin")
            if region not in regions:
                raise RegistryError(f"{where}: unknown region {region!r}; this file holds "
                                    f"{', '.join(regions)}")
            if region in seen:
                raise RegistryError(f"{where}: region {region!r} appears twice")
            seen.add(region)
            current = region
            ending = line[len(body):] or "\n"
            out += [(match["indent"] + rendered if rendered else "") + ending
                    for rendered in regions[region]]
        elif match is not None:
            if match["edge"] == "begin" or match["id"] != current:
                raise RegistryError(f"{where}: `generated:{match['edge']} {match['id']}` inside "
                                    f"region {current!r}, which has not ended")
            out.append(line)
            current = None
    if current is not None:
        raise RegistryError(f"{label}: region {current!r} never ends")
    missing = [r for r in regions if r not in seen]
    if missing:
        raise RegistryError(f"{label}: missing region{'s' if len(missing) > 1 else ''} "
                            f"{', '.join(missing)}")
    return "".join(out)


def render(registry: Registry) -> dict[str, dict[str, list[str]]]:
    return {path: {region: renderer(registry) for region, renderer in renderers.items()}
            for path, renderers in FILES.items()}


def regenerate(root: Path, registry: Registry) -> dict[str, tuple[str, str]]:
    """Each generated file under `root`: its text now, and with every region rendered."""
    out = {}
    for path, regions in render(registry).items():
        old = (root / path).read_bytes().decode("utf-8")
        out[path] = (old, splice(old, marker_style(path), regions, path))
    return out


def stale(root: Path, registry: Registry) -> str:
    """A unified diff from the committed regions to the rendered ones, or ""."""
    diff = []
    for path, (old, new) in regenerate(root, registry).items():
        diff += difflib.unified_diff(_lines(old), _lines(new), f"a/{path}", f"b/{path}")
    return "".join(diff)


def main(argv: list[str] | None = None, root: Path = REPO,
         run: publish_order.Runner = publish_order.run_command) -> int:
    parser = argparse.ArgumentParser(
        description="Render the generated regions of the docs and workflows from Cargo metadata.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="rewrite every region")
    mode.add_argument("--check", action="store_true",
                      help="exit 1 with a diff when a region differs from the metadata")
    args = parser.parse_args(argv)
    try:
        registry = load(root, run=run)
        if args.check:
            diff = stale(root, registry)
            if diff:
                sys.stdout.write(diff)
                print("::error::Generated regions differ from the Cargo metadata. Edit the "
                      "metadata, not the region, then run `python3 scripts/gen_registry.py "
                      "--write`.", file=sys.stderr)
                return 1
            return 0
        for path, (old, new) in regenerate(root, registry).items():
            if new != old:
                (root / path).write_bytes(new.encode("utf-8"))
                print(f"wrote {path}", file=sys.stderr)
    except (RegistryError, publish_order.PublishOrderError) as err:
        print(f"::error::{err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
