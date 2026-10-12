"""scripts/venue_gate.py, the CAP-7 gate: the foundation crates name no venue.

The synthetic trees cover each rule of the gate; the real-tree tests run it on the
workspace, which must pass with every exception matching something, and on
v0.39.0's core, which still held Polymarket's code and must be caught.
"""

from __future__ import annotations

import importlib.util
import io
import subprocess
import sys
import tarfile
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
BASELINE = "v0.39.0"


def _load(name: str):
    """A module from `scripts/`, which lives outside this uv project."""
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


gate = _load("venue_gate")
FOUNDATION = gate.FOUNDATION


# --- synthetic workspaces ----------------------------------------------------


def package(name: str, **meta) -> dict:
    return {"name": name, "id": f"path+file:///ws/{name}#1.0.0",
            "metadata": {"polyoxide": meta} if meta else None}


def metadata(*packages: dict) -> dict:
    return {"packages": list(packages), "workspace_members": [p["id"] for p in packages]}


def exception(path: str, term: str, reason: str = "a protected test name") -> dict:
    return {"path": path, "term": term, "reason": reason}


VENUES = metadata(
    package("polyoxide-core"),
    package("polyoxide-alpha", venue="polymarket", products=["clob", "data"]),
    package("polyoxide-beta", venue="polymarket", products=["perps", "combos-rfq"]),
    package("polyoxide-polymarket", venue="polymarket", identifiers=["Poly-RateLimit"]),
    package("polyoxide-binance", venue="binance", products=["usdm"]),
)
SKIP = ("data", "events")


def tree(root: Path, files: dict[str, str]) -> Path:
    """A workspace at `root`: every foundation crate, each with `files` of its own."""
    for crate in FOUNDATION:
        (root / crate / "src").mkdir(parents=True)
        (root / crate / "src" / "lib.rs").write_text("//! A foundation crate.\n")
    for path, text in files.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text)
    return root


def run(root: Path, meta: dict = VENUES, skip=SKIP):
    found = gate.read_gate(meta, skip)
    return gate.check(gate.scan(root, found), found.excuses)


def findings(result) -> list[tuple[str, int, str, str]]:
    return [(h.path, h.line, h.term.text, h.word) for h in result.unexcused]


def test_a_clean_tree_passes(tmp_path: Path) -> None:
    result = run(tree(tmp_path, {"polyoxide-core/src/send.rs": "// One send loop.\n"}))
    assert (result.status, result.hits, result.stale) == (0, [], [])


def test_a_venue_word_in_a_foundation_comment_fails_naming_file_line_and_term(
        tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/hooks.rs": "/// A window.\n/// As binance does.\n"})
    result = run(root)
    assert result.status == 1
    assert findings(result) == [("polyoxide-core/src/hooks.rs", 2, "binance", "binance")]
    err = io.StringIO()
    assert gate.report(result, out=io.StringIO(), err=err) == 1
    assert "::error::polyoxide-core/src/hooks.rs:2: venue 'binance'" in err.getvalue()


def test_a_venue_id_is_found_as_a_substring_in_any_case(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-ws/src/x.rs": "polymarket_limits\nPOLYMARKETS\n",
                           "polyoxide-venue/Cargo.toml": 'keywords = ["Polymarket"]\n'})
    assert sorted((p, n, t) for p, n, t, _ in findings(run(root))) == [
        ("polyoxide-venue/Cargo.toml", 1, "polymarket"),
        ("polyoxide-ws/src/x.rs", 1, "polymarket"), ("polyoxide-ws/src/x.rs", 2, "polymarket")]


@pytest.mark.parametrize(("text", "term", "word"), [
    ("fn test_clob_default_construction() {}", "clob", "test_clob_default_construction"),
    ("const PERPS_GENERAL: u32 = 30;", "perps", "PERPS_GENERAL"),
    ("struct ClobThrottle;", "clob", "ClobThrottle"),
    ("use x::perps::y;", "perps", "perps"),
    ("see the clob-ws feature", "clob", "clob-ws"),
    ("a.clob.b", "clob", "clob"),
    ("the CLOB's book", "clob", "CLOB"),
    ("two CLOBs", "clob", "CLOBs"),
    ("one CombosRfq quote", "combos-rfq", "CombosRfq"),
    ("the usdm2 path", "usdm", "usdm2"),
])
def test_a_product_is_caught_as_an_identifier_segment(tmp_path: Path, text: str, term: str,
                                                      word: str) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/x.rs": text + "\n"})
    assert findings(run(root)) == [("polyoxide-core/src/x.rs", 1, term, word)]


@pytest.mark.parametrize("text", ["clobber", "perpsx", "usdmx", "rfq alone",
                                  "combos then rfq"])
def test_a_product_inside_a_longer_segment_is_not_a_hit(tmp_path: Path, text: str) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/x.rs": text + "\n"})
    assert run(root).hits == []


@pytest.mark.parametrize("text", ["data", "the events", "DataApi", "events_stream",
                                  "fn data_events()"])
def test_a_generic_word_on_the_skip_list_is_not_a_hit(tmp_path: Path, text: str) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/x.rs": text + "\n"})
    assert run(root).hits == []
    # Without the skip list, `data` is a product like any other.
    if "data" in text.lower():
        assert any(h.term.text == "data" for h in run(root, skip=()).hits)


def test_a_venue_product_prefix_is_its_own_term(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/x.rs": 'let target = "Polymarket.Data:x";\n'})
    terms = sorted((h.term.kind, h.term.text) for h in run(root).hits)
    # `data` is skipped as a segment, but not as part of a prefix.
    assert terms == [("prefix", "polymarket.data:"), ("venue", "polymarket")]


def test_an_identifier_is_caught_in_any_case(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/x.rs": 'get("poly-ratelimit-tier")\n',
                           "polyoxide-core/README.md": "Reads `Poly-RateLimit-Tier`.\n"})
    assert sorted((p, t, w) for p, _, t, w in findings(run(root))) == [
        ("polyoxide-core/README.md", "poly-ratelimit", "Poly-RateLimit-Tier"),
        ("polyoxide-core/src/x.rs", "poly-ratelimit", "poly-ratelimit-tier")]


def test_every_file_of_every_foundation_crate_is_read_but_build_output(tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-test-support/tests/fixtures/x.json": '{"venue": "binance"}\n',
        "polyoxide-venue/examples/probe.rs": "// usdm\n",
        "polyoxide-ws/target/debug/x.rs": "// binance\n",
        "polyoxide-ws/src/target/mod.rs": "// binance\n",
        "polyoxide-other/src/lib.rs": "// binance\n",
    })
    assert sorted(h.path for h in run(root).hits) == [
        "polyoxide-test-support/tests/fixtures/x.json", "polyoxide-venue/examples/probe.rs",
        "polyoxide-ws/src/target/mod.rs"]


# --- exceptions --------------------------------------------------------------


def with_exceptions(crate: str, *entries: dict) -> dict:
    meta = metadata(*[dict(p) for p in VENUES["packages"]])
    for p in meta["packages"]:
        if p["name"] == crate:
            p["metadata"] = {"polyoxide": {**p["metadata"]["polyoxide"],
                                           "gate_exceptions": list(entries)}}
    return meta


def test_an_exception_covers_its_term_in_its_file_or_directory_only(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-ws/src/request.rs": "mod perps {}\nmod binance {}\n",
                           "polyoxide-ws/src/test_server/tests.rs": "mod as_perps {}\n",
                           "polyoxide-ws/src/test_server/mod.rs": "// perps\n",
                           "polyoxide-ws/src/other.rs": "// perps\n",
                           "polyoxide-ws/src/test_server_x.rs": "// perps\n"})
    meta = with_exceptions("polyoxide-polymarket",
                           exception("polyoxide-ws/src/request.rs", "PERPS"),
                           exception("polyoxide-ws/src/test_server", "perps"))
    result = run(root, meta)
    assert findings(result) == [("polyoxide-ws/src/other.rs", 1, "perps", "perps"),
                                ("polyoxide-ws/src/request.rs", 2, "binance", "binance"),
                                ("polyoxide-ws/src/test_server_x.rs", 1, "perps", "perps")]
    assert result.stale == []


def test_an_exception_counts_only_from_a_crate_of_the_terms_venue(tmp_path: Path) -> None:
    """Binance cannot excuse Polymarket's words, and a venue's exceptions may live in
    any crate of that venue."""
    root = tree(tmp_path, {"polyoxide-ws/src/x.rs": "mod perps {}\n"})
    result = run(root, with_exceptions("polyoxide-binance", exception("polyoxide-ws/src/x.rs",
                                                                      "perps")))
    assert [h.term.text for h in result.unexcused] == ["perps"]
    assert [(e.crate, e.term) for e in result.stale] == [("polyoxide-binance", "perps")]
    result = run(root, with_exceptions("polyoxide-beta", exception("polyoxide-ws/src/x.rs",
                                                                   "perps")))
    assert (result.status, result.stale) == (0, [])


def test_a_stale_exception_fails_naming_it(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-ws/src/x.rs": "mod perps {}\n"})
    meta = with_exceptions("polyoxide-polymarket", exception("polyoxide-ws/src/x.rs", "perps"),
                           exception("polyoxide-ws/src/x.rs", "rtds", "the old suite"))
    result = run(root, meta)
    assert result.unexcused == []
    assert [(e.path, e.term) for e in result.stale] == [("polyoxide-ws/src/x.rs", "rtds")]
    err = io.StringIO()
    assert gate.report(result, out=io.StringIO(), err=err) == 1
    assert ("polyoxide-polymarket's gate exception for 'rtds' in polyoxide-ws/src/x.rs "
            "matches nothing") in err.getvalue()


def test_a_malformed_exception_cannot_be_decided() -> None:
    meta = with_exceptions("polyoxide-polymarket", {"path": "polyoxide-ws/src/x.rs"})
    with pytest.raises(gate.GateError, match="needs a path, a term and a reason"):
        gate.read_gate(meta, SKIP)


def test_the_terms_come_from_each_venue_s_metadata() -> None:
    found = gate.read_gate(VENUES, SKIP)
    terms = {(t.kind, t.text): t.venues for t in found.terms}
    assert terms[("venue", "polymarket")] == {"polymarket"}
    assert terms[("product", "clob")] == {"polymarket"}
    assert terms[("product", "usdm")] == {"binance"}
    assert terms[("identifier", "poly-ratelimit")] == {"polymarket"}
    assert terms[("prefix", "binance.usdm:")] == {"binance"}
    assert ("product", "data") not in terms and ("prefix", "polymarket.data:") in terms


def test_main_cannot_decide_without_the_skip_list_or_metadata(tmp_path: Path, capsys) -> None:
    assert gate.main([], root=tree(tmp_path, {})) == 2
    assert "::error::" in capsys.readouterr().err


def test_a_missing_or_malformed_skip_list_cannot_be_decided(tmp_path: Path) -> None:
    with pytest.raises(gate.GateError, match="could not read"):
        gate.read_skip(tmp_path)
    (tmp_path / gate.SKIP).parent.mkdir(parents=True)
    (tmp_path / gate.SKIP).write_text("data\ntwo words\n")
    with pytest.raises(gate.GateError, match="is not one word"):
        gate.read_skip(tmp_path)


def test_a_missing_foundation_crate_cannot_be_decided(tmp_path: Path) -> None:
    root = tree(tmp_path, {})
    (root / FOUNDATION[-1] / "src" / "lib.rs").unlink()
    (root / FOUNDATION[-1] / "src").rmdir()
    (root / FOUNDATION[-1]).rmdir()
    with pytest.raises(gate.GateError, match="is not a directory"):
        gate.files(root)


def test_a_product_with_a_digit_is_caught(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-core/src/x.rs": "fn data_v2_default() {}\n"})
    meta = metadata(package("polyoxide-gamma", venue="polymarket", products=["data-v2"]))
    assert findings(run(root, meta)) == [("polyoxide-core/src/x.rs", 1, "data-v2",
                                          "data_v2_default")]


# --- the real tree -----------------------------------------------------------


def test_the_skip_list_holds_the_generic_words() -> None:
    assert gate.read_skip(REPO) == {"data", "events"}


def test_the_real_tree_passes_and_every_exception_matches() -> None:
    result = gate.run(REPO)
    assert not result.unexcused, "\n".join(map(str, result.unexcused))
    assert not result.stale, result.stale
    # Anti-vacuity: the gate read the venues, and the exceptions excuse real hits.
    found = gate.read_gate(gate.publish_order.cargo_metadata(REPO / "Cargo.toml"),
                           gate.read_skip(REPO))
    kinds = {(t.kind, t.text) for t in found.terms}
    assert {("venue", "polymarket"), ("venue", "binance"), ("product", "clob"),
            ("product", "perps"), ("identifier", "poly-ratelimit")} <= kinds
    assert result.hits and {e.crate for e in found.excuses} == {"polyoxide-polymarket",
                                                               "polyoxide-binance"}


def _baseline_core(into: Path) -> Path:
    """`polyoxide-core` as v0.39.0 shipped it."""
    try:
        archive = subprocess.run(["git", "-C", str(REPO), "archive", BASELINE, "polyoxide-core"],
                                 check=True, capture_output=True).stdout
    except subprocess.CalledProcessError as err:
        pytest.fail(f"cannot read {BASELINE} ({err.stderr.decode().strip()}); the scripts job "
                    f"checks out with `fetch-depth: 0` for it")
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(into, filter="data")
    return into


def test_v0_39_0_core_is_caught(tmp_path: Path) -> None:
    """The gate catches what Story 5.2 moved out of core: the product segments of
    `clob_default` and `PERPS_GENERAL`, and the `Poly-RateLimit` headers."""
    root = _baseline_core(tmp_path)
    found = gate.read_gate(gate.publish_order.cargo_metadata(REPO / "Cargo.toml"),
                           gate.read_skip(REPO))
    hits = gate.scan(root, found, crates=["polyoxide-core"])
    words = [h.word.lower() for h in hits]
    assert any("clob_default" in w for w in words)
    assert any(w == "perps_general" for w in words)
    assert any("poly-ratelimit" in w for w in words)
    assert len(gate.check(hits, found.excuses).unexcused) > 200
