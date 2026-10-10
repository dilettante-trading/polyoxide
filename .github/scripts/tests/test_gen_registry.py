"""Unit tests for scripts/gen_registry.py.

The real tree must pass `--check`, so a hand edit inside a generated region fails
CI here. Splicing is tested on synthetic text, validation and every renderer on a
fixture workspace built from metadata dicts and a temporary `docs/specs/`. Only
the real-tree tests run `cargo metadata --offline`, which needs a Rust toolchain
but no network.
"""

from __future__ import annotations

import importlib.util
import re
import shutil
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]


def _load_gen_registry():
    """`scripts/gen_registry.py`, which lives outside this uv project."""
    spec = importlib.util.spec_from_file_location(
        "gen_registry", REPO / "scripts" / "gen_registry.py"
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


gen_registry = _load_gen_registry()
RegistryError = gen_registry.RegistryError
GUIDE = REPO / gen_registry.GUIDE


# --- the real tree -----------------------------------------------------------


@pytest.fixture(scope="module")
def registry():
    return gen_registry.load(REPO)


def test_the_real_tree_passes_check(capsys: pytest.CaptureFixture[str]) -> None:
    assert gen_registry.main(["--check"]) == 0, capsys.readouterr().out


def _copy_generated_files(root: Path) -> None:
    for path in gen_registry.FILES:
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / path, root / path)


def _first_region_line(text: str, style: str) -> int:
    """The index of the first line inside the first region of `text`."""
    pattern = gen_registry.MARKERS[style]
    lines = text.split("\n")
    begin = next(i for i, line in enumerate(lines) if pattern.match(line))
    return begin + 1


@pytest.mark.parametrize("path", list(gen_registry.FILES))
def test_a_hand_edit_inside_a_region_fails_check(
    path: str, tmp_path: Path, registry, monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _copy_generated_files(tmp_path)
    target = tmp_path / path
    lines = target.read_text().split("\n")
    lines[_first_region_line(target.read_text(), gen_registry.marker_style(path))] += " edited"
    target.write_text("\n".join(lines))
    monkeypatch.setattr(gen_registry, "load", lambda root, run=None: registry)

    assert gen_registry.main(["--check"], root=tmp_path) == 1
    out = capsys.readouterr()
    assert f"--- a/{path}" in out.out
    assert " edited" in out.out
    assert "Edit the metadata, not the region" in out.err


def test_write_undoes_a_hand_edit_and_keeps_the_rest(
    tmp_path: Path, registry, monkeypatch: pytest.MonkeyPatch
) -> None:
    _copy_generated_files(tmp_path)
    readme = tmp_path / "README.md"
    original = readme.read_text()
    readme.write_text(original.replace("| Core utilities and shared types |", "| hand edit |"))
    monkeypatch.setattr(gen_registry, "load", lambda root, run=None: registry)

    assert gen_registry.main(["--write"], root=tmp_path) == 0
    assert readme.read_text() == original
    assert gen_registry.main(["--check"], root=tmp_path) == 0


def test_the_architecture_guide_is_the_spines_guide_byte_for_byte() -> None:
    """docs/ARCHITECTURE.md is regenerated only from the spine (AD-21). Its guide
    region is the spine's guide byte for byte, so an edit to either side alone fails
    here, and the file ends with the region."""
    text = (REPO / "docs/ARCHITECTURE.md").read_bytes()
    head, begin, rest = text.partition(b"<!-- generated:begin architecture-guide -->\n")
    body, end, tail = rest.partition(b"<!-- generated:end architecture-guide -->\n")
    assert begin and end, "docs/ARCHITECTURE.md has lost its architecture-guide region"
    assert b"Copied from the spine's guide; do not edit." in head
    assert b"<!-- generated:end architecture-stage -->" in head
    assert body == GUIDE.read_bytes()
    assert tail == b""


def test_write_copies_the_guide_into_the_architecture_file(
    tmp_path: Path, registry, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Copying the guide is a generator step, not a hand splice under a do-not-edit
    header."""
    _copy_generated_files(tmp_path)
    target = tmp_path / "docs/ARCHITECTURE.md"
    original = target.read_bytes()
    head, begin, rest = original.partition(b"<!-- generated:begin architecture-guide -->\n")
    _, end, tail = rest.partition(b"<!-- generated:end architecture-guide -->\n")
    target.write_bytes(head + begin + end + tail)
    monkeypatch.setattr(gen_registry, "load", lambda root, run=None: registry)

    assert gen_registry.main(["--write"], root=tmp_path) == 0
    assert target.read_bytes() == original


def _anchor(heading: str) -> str:
    """GitHub's anchor for a Markdown heading's text."""
    return re.sub(r"[^\w\- ]", "", heading.strip().lower()).replace(" ", "-")


def test_the_stage_line_links_a_heading_the_guide_has() -> None:
    headings = [line.lstrip("#") for line in GUIDE.read_text().splitlines()
                if re.match(r"^#{1,6} ", line)]
    assert gen_registry.STAGE_ANCHOR in {_anchor(h) for h in headings}
    assert f"(#{gen_registry.STAGE_ANCHOR})" in gen_registry.architecture_stage(
        gen_registry.load(REPO))[0]


def test_check_and_write_are_required_and_exclusive() -> None:
    with pytest.raises(SystemExit) as missing:
        gen_registry.main([])
    with pytest.raises(SystemExit) as both:
        gen_registry.main(["--check", "--write"])
    assert missing.value.code == both.value.code == 2


def test_refused_metadata_exits_1_with_the_reason(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def refuse(root, run=None):
        raise RegistryError("(polymarket, clob) is declared by more than one crate")

    monkeypatch.setattr(gen_registry, "load", refuse)
    assert gen_registry.main(["--check"]) == 1
    assert "::error::(polymarket, clob)" in capsys.readouterr().err


# --- splicing ----------------------------------------------------------------


def test_text_outside_markers_is_kept_byte_for_byte() -> None:
    before = "head \t  \r\nü — unicode\n\n<!-- generated:begin one -->\n"
    middle = "<!-- generated:end one -->\r\n  between  \n<!-- generated:begin two -->\n"
    after = "<!-- generated:end two -->\nno final newline"
    text = before + "stale\nstale\n" + middle + "old\n" + after

    out = gen_registry.splice(text, "markdown", {"one": ["fresh"], "two": []})

    assert out == before + "fresh\n" + middle + after


def test_a_crlf_begin_marker_gives_its_lines_crlf() -> None:
    text = "<!-- generated:begin one -->\r\nold\r\n<!-- generated:end one -->\r\n"
    out = gen_registry.splice(text, "markdown", {"one": ["a", "b"]})
    assert out == "<!-- generated:begin one -->\r\na\r\nb\r\n<!-- generated:end one -->\r\n"


def test_indentation_is_kept() -> None:
    text = "jobs:\n    # generated:begin jobs\n    old: 1\n    # generated:end jobs\n"
    out = gen_registry.splice(text, "hash", {"jobs": ["a:", "  b: 1", "", "c:"]})
    # A blank generated line stays blank: no trailing whitespace.
    assert out == ("jobs:\n    # generated:begin jobs\n    a:\n      b: 1\n\n    c:\n"
                   "    # generated:end jobs\n")


BEGIN_A, END_A = "# generated:begin a\n", "# generated:end a\n"


@pytest.mark.parametrize(
    ("text", "message"),
    [
        (BEGIN_A, "file.yml: region 'a' never ends"),
        (END_A, "file.yml:1: `generated:end a` has no begin"),
        (BEGIN_A + END_A + BEGIN_A + END_A, "file.yml:3: region 'a' appears twice"),
        (BEGIN_A + "# generated:begin b\n", "file.yml:2: `generated:begin b` inside region 'a'"),
        (BEGIN_A + "# generated:end b\n", "file.yml:2: `generated:end b` inside region 'a'"),
        ("# generated:begin z\n# generated:end z\n", "file.yml:1: unknown region 'z'"),
        (BEGIN_A + END_A, "file.yml: missing region b"),
    ],
)
def test_bad_markers_are_refused(text: str, message: str) -> None:
    with pytest.raises(RegistryError) as err:
        gen_registry.splice(text, "hash", {"a": [], "b": []}, "file.yml")
    assert message in str(err.value)


def test_each_file_uses_its_comment_leader() -> None:
    assert gen_registry.marker_style("README.md") == "markdown"
    assert gen_registry.marker_style(".github/workflows/nightly-schema.yml") == "hash"
    assert gen_registry.marker_style("Cargo.toml") == "hash"
    # A Markdown marker in a YAML file is just text.
    text = "<!-- generated:begin a -->\n# generated:begin a\n# generated:end a\n"
    assert gen_registry.splice(text, "hash", {"a": ["x"]}).startswith("<!-- generated:begin a -->\n")


# --- the fixture workspace ---------------------------------------------------
#
# polyoxide-core      no deps
# polyoxide-alpha     venue va, product a; mirrors alpha and hidden; three live targets
# polyoxide-omega     venue vo, product o; mirror omega; a versioned dev-dependency on alpha
# polyoxide           the umbrella, alpha optional
# polyoxide-cli       alpha with `ws`, omega, and core under `keychain`
# polyoxide-py        unpublished, alpha without default features


def dep(name: str, kind: str | None = None, optional: bool = False,
        features: tuple[str, ...] = (), default_features: bool = True,
        req: str = "^1.0.0") -> dict:
    return {"name": name, "kind": kind, "req": req, "path": f"/ws/{name}",
            "optional": optional, "features": list(features),
            "uses_default_features": default_features}


def package(root: Path, name: str, *deps: dict, publish: list | None = None) -> dict:
    return {"name": name, "version": "1.0.0", "id": f"path+file:///ws/{name}#1.0.0",
            "publish": publish, "manifest_path": str(root / name / "Cargo.toml"),
            "dependencies": list(deps), "targets": []}


def live(suite: str = "live", timeout: int = 15, features=(), secrets=(), note=None) -> dict:
    entry = {"suite": suite, "timeout": timeout, "features": list(features),
             "secrets": list(secrets)}
    if note:
        entry["note"] = note
    return entry


def manifest(features: dict | None = None, **meta) -> dict:
    return {"features": features or {}, "package": {"metadata": {"polyoxide": meta}}}


def spec(id: str, kind: str, vendored: str, **extra) -> dict:
    return {"id": id, "kind": kind, "vendored": vendored, **extra}


FIXTURE_GUIDE = "# A guide\n\n### Which stage the workspace is in (AD-16)\n\n  indented text \n"


def fixture(root: Path) -> tuple[dict, dict, dict]:
    """(cargo metadata, root manifest, member manifests) for the fixture workspace."""
    (root / gen_registry.GUIDE).parent.mkdir(parents=True)
    (root / gen_registry.GUIDE).write_text(FIXTURE_GUIDE)
    for directory, files in {
        "alpha": ["INDEX.md", "openapi.yaml", "asyncapi.json"],
        "beta": ["INDEX.md", "openapi.yaml"],
        "omega": ["README.md"],
        "hidden": ["OBSERVED.md"],
    }.items():
        (root / "docs/specs" / directory).mkdir(parents=True)
        for file in files:
            (root / "docs/specs" / directory / file).write_text("{}")
    packages = [
        package(root, "polyoxide", dep("polyoxide-alpha", optional=True)),
        package(root, "polyoxide-core"),
        package(root, "polyoxide-alpha", dep("polyoxide-core")),
        package(root, "polyoxide-omega", dep("polyoxide-core"),
                dep("polyoxide-alpha", kind="dev")),
        package(root, "polyoxide-cli", dep("polyoxide-alpha", features=("ws",)),
                dep("polyoxide-omega"),
                dep("polyoxide-core", optional=True, features=("keychain",)),
                dep("polyoxide-omega", kind="dev", req="*")),
        package(root, "polyoxide-py", dep("polyoxide-alpha", default_features=False, req="*"),
                publish=[]),
    ]
    metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}
    mirrors = {
        "alpha": {
            "name": "Alpha", "section": "covered", "base_urls": ["https://alpha.test"],
            "description": "Alpha things", "crate_note": "`alpha()`",
            "specs": [
                spec("alpha", "openapi", "docs/specs/alpha/openapi.yaml",
                     url="https://docs.test/alpha.yaml", note="served by the host"),
                spec("alpha-ws", "asyncapi", "docs/specs/alpha/asyncapi.json",
                     url="https://docs.test/alpha.json", covers="Alpha channel (2 messages)"),
            ],
        },
        "beta": {
            "name": "Beta", "section": "not-implemented", "base_urls": ["https://beta.test"],
            "description": "Beta things",
            "specs": [spec("beta", "openapi", "docs/specs/beta/openapi.yaml",
                           url="https://docs.test/beta.yaml")],
        },
        "hidden": {
            "name": "Hidden hosts", "section": "excluded", "base_urls": ["https://hidden.test"],
            "description": "Undocumented", "exclude": "Nothing is published to diff against.",
        },
        "omega": {
            "name": "Omega", "section": "other-venue",
            "base_urls": ["https://omega.test", "wss://omega.test"],
            "description": "Another venue", "exclude": "Omega publishes no spec.",
        },
    }
    workspace = {"workspace": {"metadata": {"polyoxide": {"stage": "S1", "mirrors": mirrors}}}}
    manifests = {
        "polyoxide": manifest(
            {"default": ["alpha"], "alpha": ["dep:polyoxide-alpha"],
             "alpha-ws": ["alpha", "polyoxide-alpha/ws"], "full": ["alpha", "alpha-ws"],
             "extra": ["polyoxide-alpha?/extra"]},
            readme="The umbrella"),
        "polyoxide-core": manifest(readme="Shared things", notes=["the base"]),
        "polyoxide-alpha": manifest(
            readme="Alpha client", venue="va", products=["a"], mirrors=["alpha", "hidden"],
            live={"live_ws": live(features=["ws"], secrets=["B_KEY"]),
                  "live_api": live(secrets=["A_KEY", "B_KEY"]),
                  "live_slow": live(suite="slow", timeout=30,
                                    note="30-minute budget for a slow host")}),
        "polyoxide-omega": manifest(readme="Omega client", venue="vo", products=["o"],
                                    mirrors=["omega"], live={"live_api": live()}),
        "polyoxide-cli": manifest({"keychain": ["dep:polyoxide-core"]}, readme="The CLI"),
        "polyoxide-py": manifest(readme="Bindings"),
    }
    return metadata, workspace, manifests


def build(root: Path, edit=None):
    metadata, workspace, manifests = fixture(root)
    if edit:
        edit(metadata, workspace, manifests)
    return gen_registry.build(root, metadata, workspace, manifests)


@pytest.fixture
def fixture_registry(tmp_path: Path):
    return build(tmp_path)


# --- validation --------------------------------------------------------------


def test_the_fixture_orders_crates_by_publish_order_then_unpublished(fixture_registry) -> None:
    assert [c.name for c in fixture_registry.crates] == [
        "polyoxide-core", "polyoxide-alpha", "polyoxide", "polyoxide-omega",
        "polyoxide-cli", "polyoxide-py"]


def test_a_duplicate_venue_and_product_fails_naming_both_crates(tmp_path: Path) -> None:
    def clash(metadata, workspace, manifests):
        meta = manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]
        meta["venue"], meta["products"] = "va", ["a"]

    with pytest.raises(RegistryError) as err:
        build(tmp_path, clash)
    assert "(va, a)" in str(err.value)
    assert "polyoxide-alpha" in str(err.value) and "polyoxide-omega" in str(err.value)


def test_one_venue_may_have_many_products_across_crates(tmp_path: Path) -> None:
    def share(metadata, workspace, manifests):
        manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["venue"] = "va"

    assert build(tmp_path, share).crate("polyoxide-omega").venue == "va"


@pytest.mark.parametrize(
    ("key", "value"),
    [("venue", "Va"), ("venue", "va_x"), ("products", ["a", "1b"]), ("products", ["-a"])],
)
def test_a_bad_id_fails(tmp_path: Path, key: str, value) -> None:
    def bad(metadata, workspace, manifests):
        manifests["polyoxide-alpha"]["package"]["metadata"]["polyoxide"][key] = value

    with pytest.raises(RegistryError, match="is not an id"):
        build(tmp_path, bad)


def test_a_bad_suite_id_fails(tmp_path: Path) -> None:
    def bad(metadata, workspace, manifests):
        live_table = manifests["polyoxide-alpha"]["package"]["metadata"]["polyoxide"]["live"]
        live_table["live_api"]["suite"] = "Live API"

    with pytest.raises(RegistryError, match="suite 'Live API' is not an id"):
        build(tmp_path, bad)


def test_a_venue_without_products_fails(tmp_path: Path) -> None:
    def half(metadata, workspace, manifests):
        del manifests["polyoxide-alpha"]["package"]["metadata"]["polyoxide"]["products"]

    with pytest.raises(RegistryError, match="give both or neither"):
        build(tmp_path, half)


def test_an_unknown_mirror_fails(tmp_path: Path) -> None:
    def unknown(metadata, workspace, manifests):
        manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["mirrors"] = ["nope"]

    with pytest.raises(RegistryError, match="polyoxide-omega lists mirror 'nope'"):
        build(tmp_path, unknown)


def test_a_mirror_whose_directory_is_missing_fails(tmp_path: Path) -> None:
    def ghost(metadata, workspace, manifests):
        mirrors = workspace["workspace"]["metadata"]["polyoxide"]["mirrors"]
        mirrors["ghost"] = dict(mirrors["beta"], specs=[])

    with pytest.raises(RegistryError, match="docs/specs/ghost/ does not exist"):
        build(tmp_path, ghost)


def test_a_directory_no_mirror_declares_fails(tmp_path: Path) -> None:
    (tmp_path / "docs/specs/stray").mkdir(parents=True)
    with pytest.raises(RegistryError, match="no mirror entry declares: stray"):
        build(tmp_path)


def test_a_member_without_registration_fails(tmp_path: Path) -> None:
    def bare(metadata, workspace, manifests):
        manifests["polyoxide-omega"]["package"] = {}

    with pytest.raises(RegistryError, match=r"polyoxide-omega: \[package.metadata.polyoxide\] is missing"):
        build(tmp_path, bare)


def test_an_unknown_key_fails(tmp_path: Path) -> None:
    def typo(metadata, workspace, manifests):
        manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["mirror"] = ["omega"]

    with pytest.raises(RegistryError, match="unknown key mirror"):
        build(tmp_path, typo)


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("timeout", 0, "positive number of minutes"),
        ("secrets", ["lower_case"], "not a repository secret name"),
        ("secrets", ["GITHUB_TOKEN"], "not a repository secret name"),
        ("note", "a 40-minute budget", "note says 40 minutes, but timeout is 15"),
    ],
)
def test_a_bad_live_entry_fails(tmp_path: Path, field: str, value, message: str) -> None:
    def bad(metadata, workspace, manifests):
        manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]["live_api"][field] = value

    with pytest.raises(RegistryError, match=message):
        build(tmp_path, bad)


def test_a_live_entry_must_declare_its_secrets(tmp_path: Path) -> None:
    def bare(metadata, workspace, manifests):
        del manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]["live_api"]["secrets"]

    with pytest.raises(RegistryError, match="lacks secrets"):
        build(tmp_path, bare)


@pytest.mark.parametrize(
    ("mirror", "change", "message"),
    [
        ("beta", {"section": "elsewhere"}, "is not one of"),
        ("hidden", {"exclude": None}, "without an `exclude` reason"),
        ("beta", {"section": "covered"}, "no crate lists it"),
        ("alpha", {"section": "not-implemented"}, "polyoxide-alpha list it"),
    ],
)
def test_a_mirror_in_the_wrong_section_fails(tmp_path: Path, mirror: str, change: dict,
                                             message: str) -> None:
    def edit(metadata, workspace, manifests):
        entry = workspace["workspace"]["metadata"]["polyoxide"]["mirrors"][mirror]
        entry.update(change)
        if entry.get("exclude") is None:
            entry.pop("exclude", None)

    with pytest.raises(RegistryError, match=message):
        build(tmp_path, edit)


def test_a_watched_spec_needs_a_url(tmp_path: Path) -> None:
    def no_url(metadata, workspace, manifests):
        del workspace["workspace"]["metadata"]["polyoxide"]["mirrors"]["beta"]["specs"][0]["url"]

    with pytest.raises(RegistryError, match="has no url"):
        build(tmp_path, no_url)


def test_a_spec_outside_its_directory_fails(tmp_path: Path) -> None:
    def moved(metadata, workspace, manifests):
        specs = workspace["workspace"]["metadata"]["polyoxide"]["mirrors"]["beta"]["specs"]
        specs[0]["vendored"] = "docs/specs/alpha/openapi.yaml"

    with pytest.raises(RegistryError, match="is not a file in docs/specs/beta/"):
        build(tmp_path, moved)


def test_a_spec_id_is_unique(tmp_path: Path) -> None:
    def twice(metadata, workspace, manifests):
        workspace["workspace"]["metadata"]["polyoxide"]["mirrors"]["beta"]["specs"][0]["id"] = "alpha"

    with pytest.raises(RegistryError, match="spec id 'alpha' is declared twice"):
        build(tmp_path, twice)


@pytest.mark.parametrize("stage", [None, "s1", "S0", "S", "stage 1", 1, "S1\n", " S1", "S1 "])
def test_a_missing_or_bad_stage_fails(tmp_path: Path, stage) -> None:
    def bad(metadata, workspace, manifests):
        meta = workspace["workspace"]["metadata"]["polyoxide"]
        if stage is None:
            del meta["stage"]
        else:
            meta["stage"] = stage

    with pytest.raises(RegistryError, match="is not a release stage"):
        build(tmp_path, bad)


def test_a_guide_without_a_final_newline_fails(tmp_path: Path) -> None:
    def chop(metadata, workspace, manifests):
        (tmp_path / gen_registry.GUIDE).write_text(FIXTURE_GUIDE.rstrip("\n"))

    with pytest.raises(RegistryError, match="must end with a newline"):
        build(tmp_path, chop)


def test_an_unknown_workspace_key_fails(tmp_path: Path) -> None:
    def typo(metadata, workspace, manifests):
        workspace["workspace"]["metadata"]["polyoxide"]["stages"] = "S2"

    with pytest.raises(RegistryError, match=r"\[workspace.metadata.polyoxide\]: unknown key stages"):
        build(tmp_path, typo)


def test_a_spec_level_exclusion_is_listed_and_not_watched(tmp_path: Path) -> None:
    def exclude(metadata, workspace, manifests):
        specs = workspace["workspace"]["metadata"]["polyoxide"]["mirrors"]["alpha"]["specs"]
        specs[1]["exclude"] = "The socket is not published."

    registry = build(tmp_path, exclude)
    assert [s.id for _, s in registry.watched()] == ["alpha", "beta"]
    # In mirror order, so alpha's spec comes before the hidden and omega directories.
    assert registry.exclusions()[0] == gen_registry.Exclusion(
        "alpha-ws", "docs/specs/alpha/asyncapi.json", "The socket is not published.")


# --- the jobs ----------------------------------------------------------------


def test_one_job_per_crate_and_suite(fixture_registry) -> None:
    jobs = fixture_registry.jobs()
    assert [(j.id, j.timeout, j.flags, j.secrets) for j in jobs] == [
        ("live-polyoxide-alpha-live", 15, "--features ws --test live_api --test live_ws",
         ("A_KEY", "B_KEY")),
        ("live-polyoxide-alpha-slow", 30, "--test live_slow", ()),
        ("live-polyoxide-omega-live", 15, "--test live_api", ()),
    ]


# --- the renderers -----------------------------------------------------------

EXPECTED = {
    "readme-crates": [
        "| Crate | Description |",
        "|-------|-------------|",
        "| [polyoxide](./polyoxide) | The umbrella |",
        "| [polyoxide-alpha](./polyoxide-alpha) | Alpha client |",
        "| [polyoxide-cli](./polyoxide-cli) | The CLI |",
        "| [polyoxide-core](./polyoxide-core) | Shared things |",
        "| [polyoxide-omega](./polyoxide-omega) | Omega client |",
        "| [polyoxide-py](./polyoxide-py) | Bindings |",
    ],
    "index-upstream": [
        "- Alpha: https://docs.test/alpha.yaml (served by the host)",
        "- Beta: https://docs.test/beta.yaml",
    ],
    "index-covered": [
        "| API | Base URL | Description | Crate |",
        "|-----|----------|-------------|-------|",
        "| [Alpha](alpha/INDEX.md) | `https://alpha.test` | Alpha things | `polyoxide-alpha` (`alpha()`) |",
    ],
    "index-not-implemented": [
        "| API | Base URL | Description |",
        "|-----|----------|-------------|",
        "| [Beta](beta/INDEX.md) | `https://beta.test` | Beta things |",
    ],
    "index-other-venues": [
        "| API | Base URL | Description | Crate |",
        "|-----|----------|-------------|-------|",
        "| [Omega](omega/README.md) | `https://omega.test`, `wss://omega.test` | Another venue | `polyoxide-omega` |",
    ],
    "index-asyncapi": [
        "| Spec | Covers | Crate |",
        "|------|--------|-------|",
        "| [alpha/asyncapi.json](alpha/asyncapi.json) | Alpha channel (2 messages) | `polyoxide-alpha` |",
    ],
    "claude-crate-count": [
        "Six crates, in publish order, each with the workspace crates its build needs. "
        "Crates that are not published come last:",
    ],
    "claude-graph": [
        "- `polyoxide-core` — Shared things; needs: nothing in the workspace; the base",
        "- `polyoxide-alpha` — Alpha client; needs: `polyoxide-core`",
        "- `polyoxide` — The umbrella; needs: `polyoxide-alpha` (under `alpha`, on by default)",
        "- `polyoxide-omega` — Omega client; needs: `polyoxide-core`; published after "
        "`polyoxide-alpha`, a versioned dev-dependency",
        "- `polyoxide-cli` — The CLI; needs: `polyoxide-alpha` (with `ws`), "
        "`polyoxide-core` (with `keychain`, under `keychain`), `polyoxide-omega`",
        "- `polyoxide-py` — Bindings; needs: `polyoxide-alpha` (without default features)",
    ],
    "claude-umbrella-features": [
        "**polyoxide** (the unified crate) uses feature flags: `alpha`, "
        "`alpha-ws` (`polyoxide-alpha/ws`), `full` (all but `extra`), "
        "`extra` (`polyoxide-alpha?/extra`). Default = alpha.",
    ],
    "claude-cli-deps": [
        "Note: `polyoxide-cli` does **not** depend on the unified `polyoxide` crate. It "
        "depends directly on the component crates — `polyoxide-alpha` (with `ws`) and "
        "`polyoxide-omega` — plus `polyoxide-core` (with `keychain`) only under the "
        "optional `keychain` feature.",
    ],
    "claude-publish-order": [
        "Today's order: `polyoxide-core` → `polyoxide-alpha` → `polyoxide` → "
        "`polyoxide-omega` → `polyoxide-cli`.",
    ],
    "claude-nightly": [
        "- `live-polyoxide-alpha-live` (15 min): `live_api`, `live_ws` with `--features ws`; "
        "secrets `A_KEY`, `B_KEY`",
        "- `live-polyoxide-alpha-slow` (30 min): `live_slow`",
        "- `live-polyoxide-omega-live` (15 min): `live_api`",
    ],
    "claude-schema-watch": [
        "It watches two OpenAPI (`alpha`, `beta`) and one AsyncAPI (`alpha-ws`), each filed "
        "under its own `spec:<id>` label.",
    ],
    "claude-schema-exclusions": [
        "- Hidden hosts (`docs/specs/hidden/`): Nothing is published to diff against.",
        "- Omega (`docs/specs/omega/`): Omega publishes no spec.",
    ],
    "selfheal-behavioral": [
        "| Crate | Test binaries |",
        "|-------|---------------|",
        "| polyoxide-alpha | `live`: `live_api`, `live_ws` (built with `--features ws`); "
        "`slow`: `live_slow` (30-minute budget for a slow host) |",
        "| polyoxide-omega | `live_api` |",
    ],
    "selfheal-watch": [
        "| Entry | Upstream | Vendored mirror |",
        "|-------|----------|-----------------|",
        "| alpha | `docs.test/alpha.yaml` (served by the host) | `docs/specs/alpha/openapi.yaml` |",
        "| beta | `docs.test/beta.yaml` | `docs/specs/beta/openapi.yaml` |",
        "| alpha-ws | `docs.test/alpha.json` | `docs/specs/alpha/asyncapi.json` |",
    ],
    "selfheal-exclusions": [
        "- **Hidden hosts** (`docs/specs/hidden/`) — Nothing is published to diff",
        "  against.",
        "- **Omega** (`docs/specs/omega/`) — Omega publishes no spec.",
    ],
    "nightly-live-jobs": [
        "live-polyoxide-alpha-live:",
        "  name: Live tests (polyoxide-alpha, live)",
        "  runs-on: ubuntu-latest",
        "  timeout-minutes: 15",
        "  env:",
        "    A_KEY: ${{ secrets.A_KEY }}",
        "    B_KEY: ${{ secrets.B_KEY }}",
        "  steps:",
        "    - uses: actions/checkout@v5",
        "    - uses: ./.github/actions/live-suite",
        "      with:",
        "        crate: polyoxide-alpha",
        "        suite: live",
        '        flags: "--features ws --test live_api --test live_ws"',
        "",
        "live-polyoxide-alpha-slow:",
        "  name: Live tests (polyoxide-alpha, slow)",
        "  runs-on: ubuntu-latest",
        "  timeout-minutes: 30",
        "  steps:",
        "    - uses: actions/checkout@v5",
        "    - uses: ./.github/actions/live-suite",
        "      with:",
        "        crate: polyoxide-alpha",
        "        suite: slow",
        '        flags: "--test live_slow"',
        "",
        "live-polyoxide-omega-live:",
        "  name: Live tests (polyoxide-omega, live)",
        "  runs-on: ubuntu-latest",
        "  timeout-minutes: 15",
        "  steps:",
        "    - uses: actions/checkout@v5",
        "    - uses: ./.github/actions/live-suite",
        "      with:",
        "        crate: polyoxide-omega",
        "        suite: live",
        '        flags: "--test live_api"',
    ],
    "nightly-aggregate-needs": [
        "needs:",
        "  - live-polyoxide-alpha-live",
        "  - live-polyoxide-alpha-slow",
        "  - live-polyoxide-omega-live",
    ],
    "schema-watch": [
        '- { id: alpha,    url: "https://docs.test/alpha.yaml", vendored: docs/specs/alpha/openapi.yaml }',
        '- { id: beta,     url: "https://docs.test/beta.yaml",  vendored: docs/specs/beta/openapi.yaml }',
        '- { id: alpha-ws, url: "https://docs.test/alpha.json", vendored: docs/specs/alpha/asyncapi.json }',
    ],
    "schema-exclusions": [
        "#   - Hidden hosts (docs/specs/hidden/): Nothing is published to diff",
        "#     against.",
        "#   - Omega (docs/specs/omega/): Omega publishes no spec.",
    ],
    "architecture-stage": [
        "**The workspace is in stage S1.** [Which stage the workspace is in]"
        "(#which-stage-the-workspace-is-in-ad-16) says what each stage changes.",
    ],
    "architecture-guide": ["# A guide", "", "### Which stage the workspace is in (AD-16)", "",
                           "  indented text "],
}

RENDERERS = {region: renderer for regions in gen_registry.FILES.values()
             for region, renderer in regions.items()}


def test_every_renderer_has_an_expectation() -> None:
    assert set(RENDERERS) == set(EXPECTED)


@pytest.mark.parametrize("region", list(EXPECTED))
def test_each_renderer_on_fixture_metadata(region: str, fixture_registry) -> None:
    assert RENDERERS[region](fixture_registry) == EXPECTED[region]


def test_long_exclusions_wrap_under_their_bullet(tmp_path: Path) -> None:
    reason = " ".join(["word"] * 40)

    def long(metadata, workspace, manifests):
        workspace["workspace"]["metadata"]["polyoxide"]["mirrors"]["omega"]["exclude"] = reason

    registry = build(tmp_path, long)
    markdown = gen_registry.selfheal_exclusions(registry)
    assert all(len(line) <= 80 for line in markdown)
    assert all(line.startswith(("- ", "  ")) for line in markdown)
    yaml_lines = gen_registry.schema_exclusions(registry)
    assert all(line.startswith(("#   - ", "#     ")) for line in yaml_lines)
    omega = next(i for i, line in enumerate(yaml_lines) if line.startswith("#   - Omega"))
    assert " ".join(line[6:] for line in yaml_lines[omega:]) == f"Omega (docs/specs/omega/): {reason}"


def test_a_cli_that_needs_the_umbrella_says_so(tmp_path: Path) -> None:
    def umbrella(metadata, workspace, manifests):
        cli = next(p for p in metadata["packages"] if p["name"] == "polyoxide-cli")
        cli["dependencies"].append(dep("polyoxide"))

    line = gen_registry.claude_cli_deps(build(tmp_path, umbrella))[0]
    assert line.startswith("Note: `polyoxide-cli` depends on the unified `polyoxide` crate.")
    assert "`polyoxide`," not in line
