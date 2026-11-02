"""scripts/runtime_gate.py, the gate that holds time and tasks to the injected runtime (D3).

The synthetic trees cover each banned path, each skip and the opt-out. The real-tree tests
run the gate on the workspace, which must be clean, and prove it reads every scanned crate
and that the two wall-clock reads in tree are exempt by their opt-outs.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
CORE = "polyoxide-core/src/x.rs"
BAD = "fn f() { tokio::spawn(work()); }\n"


def _load(name: str):
    """A module from `scripts/`, which lives outside this uv project."""
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


gate = _load("runtime_gate")


def tree(root: Path, files: dict[str, str]) -> Path:
    for path, text in files.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text)
    return root


def found(root: Path, **options) -> list[tuple[str, int, str]]:
    return [(h.path, h.line, h.banned) for h in gate.scan(root, **options)]


# --- the banned paths ----------------------------------------------------------


@pytest.mark.parametrize(("code", "banned"), [
    ("tokio::spawn(async {});", "tokio::spawn"),
    ("tokio::task::spawn(work());", "tokio::task::spawn"),
    ("tokio::time::sleep(wait).await;", "tokio::time::"),
    ("use tokio::time::{sleep, Instant};", "tokio::time::"),
    ("let started = Instant::now();", "Instant::now("),
    ("let started = std::time::Instant::now();", "Instant::now("),
    ("let at = web_time::SystemTime::now ();", "SystemTime::now("),
])
def test_each_banned_path_is_a_hit(tmp_path: Path, code: str, banned: str) -> None:
    root = tree(tmp_path, {CORE: f"fn f() {{\n    {code}\n}}\n"})
    assert found(root) == [(CORE, 2, banned)]


@pytest.mark.parametrize("crate", gate.CRATES)
def test_a_hit_is_found_in_each_scanned_crate(tmp_path: Path, crate: str) -> None:
    path = f"{crate}/src/deep/x.rs"
    assert found(tree(tmp_path, {path: BAD})) == [(path, 1, "tokio::spawn")]


def test_a_path_that_spells_two_banned_paths_is_two_hits(tmp_path: Path) -> None:
    root = tree(tmp_path, {CORE: "let t = tokio::time::Instant::now();\n"})
    assert found(root) == [(CORE, 1, "Instant::now("), (CORE, 1, "tokio::time::")]


def test_what_is_not_a_scanned_crates_source_is_not_read(tmp_path: Path) -> None:
    line = "fn f() { tokio::spawn(work()); let _ = Instant::now(); }\n"
    paths = ("polyoxide-rt/src/tokio.rs", "polyoxide-cli/src/main.rs",
             "polyoxide-test-support/src/lib.rs", "polyoxide-core/tests/send.rs",
             "polyoxide-core/examples/probe.rs", "polyoxide-core/benches/send.rs",
             "polyoxide-core/build.rs")
    assert found(tree(tmp_path, {path: line for path in paths})) == []


def test_code_that_goes_through_the_runtime_is_clean(tmp_path: Path) -> None:
    text = (
        "async fn f(runtime: &dyn Runtime) {\n"
        "    runtime.sleep(wait).await;\n"
        "    let now = runtime.now();\n"
        "    let task = polyoxide_rt::spawn(runtime, work());\n"
        "    let _ = polyoxide_rt::timeout(runtime, wait, work()).await;\n"
        "}\n"
    )
    assert found(tree(tmp_path, {CORE: text})) == []


def test_comments_and_string_literals_are_not_code(tmp_path: Path) -> None:
    text = (
        "// tokio::spawn(task);\n"
        "/// Calls `Instant::now()` once.\n"
        'let s = "SystemTime::now() and tokio::time::sleep";\n'
        'let r = r#"Instant::now()"#;\n'
        "/* tokio::task::spawn(x) */\n"
    )
    assert found(tree(tmp_path, {CORE: text})) == []


# --- what the gate skips -------------------------------------------------------

AFTER_A_TEST_MODULE = (
    "pub fn before() {}\n"
    "\n"
    "#[cfg(test)]\n"
    "mod tests {\n"
    "    use super::*;\n"
    "    #[tokio::test]\n"
    "    async fn t() {\n"
    "        tokio::time::sleep(D).await;\n"
    "        let _ = Instant::now();\n"
    "    }\n"
    "}\n"
    "\n"
    "pub fn after() {\n"
    "    let _ = Instant::now();\n"
    "}\n"
)


def test_a_cfg_test_module_is_skipped_and_the_code_after_it_is_not(tmp_path: Path) -> None:
    assert found(tree(tmp_path, {CORE: AFTER_A_TEST_MODULE})) == [(CORE, 14, "Instant::now(")]


@pytest.mark.parametrize(("predicate", "skipped"), [
    ("test", True),
    ('any(test, feature = "scripted")', True),
    ('all(test, feature = "x")', True),
    ("not(test)", False),
    ('feature = "test-server"', False),
    ('any(not(test), feature = "x")', False),
    ('target_arch = "wasm32"', False),
])
def test_only_a_cfg_naming_test_outside_not_skips_a_module(
        tmp_path: Path, predicate: str, skipped: bool) -> None:
    text = f"#[cfg({predicate})]\nmod m {{\n    fn f() {{ tokio::spawn(work()); }}\n}}\n"
    expected = [] if skipped else [(CORE, 3, "tokio::spawn")]
    assert found(tree(tmp_path, {CORE: text})) == expected


def test_a_cfg_test_use_and_fn_are_skipped_and_the_next_item_is_not(tmp_path: Path) -> None:
    text = (
        "#[cfg(test)]\n"
        "use tokio::time::{sleep, Instant};\n"
        "#[cfg(test)]\n"
        "fn helper() -> Result<(), E> {\n"
        "    tokio::spawn(work());\n"
        "    Ok(())\n"
        "}\n"
        "use tokio::time::sleep;\n"
    )
    assert found(tree(tmp_path, {CORE: text})) == [(CORE, 8, "tokio::time::")]


def test_the_file_a_cfg_test_mod_declares_is_skipped_beside_lib_and_mod_rs(
        tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-core/src/lib.rs": "#[cfg(test)]\nmod tests;\nmod live;\n",
        "polyoxide-core/src/tests.rs": BAD,
        "polyoxide-core/src/tests/inner.rs": BAD,
        "polyoxide-core/src/live.rs": BAD,
        "polyoxide-ws/src/supervisor/mod.rs": "#[cfg(test)]\nmod tests;\n",
        "polyoxide-ws/src/supervisor/tests.rs": BAD,
    })
    assert found(root) == [("polyoxide-core/src/live.rs", 1, "tokio::spawn")]


def test_the_file_a_plain_file_declares_is_in_its_own_directory(tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-core/src/send.rs": "#[cfg(test)]\nmod tests;\n",
        "polyoxide-core/src/send/tests.rs": BAD,
        "polyoxide-core/src/tests.rs": BAD,
    })
    assert found(root) == [("polyoxide-core/src/tests.rs", 1, "tokio::spawn")]


def test_an_inner_cfg_test_before_the_first_item_skips_the_file_and_its_module(
        tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-core/src/helper.rs": "#![cfg(test)]\n" + BAD,
        "polyoxide-core/src/helper/more.rs": BAD,
        "polyoxide-core/src/real.rs": "#![allow(dead_code)]\n" + BAD,
        "polyoxide-core/src/late.rs": BAD + "mod m {\n    #![cfg(test)]\n}\n",
    })
    assert found(root) == [("polyoxide-core/src/late.rs", 1, "tokio::spawn"),
                           ("polyoxide-core/src/real.rs", 2, "tokio::spawn")]


def test_the_test_server_and_every_scripted_fixture_are_skipped(tmp_path: Path) -> None:
    skipped = ("polyoxide-ws/src/test_server/mod.rs", "polyoxide-ws/src/test_server/helpers.rs",
               "polyoxide-binance/src/usdm/ws/scripted.rs",
               "polyoxide-kalshi/src/predictions/ws/scripted.rs")
    files = {path: BAD for path in skipped}
    # A test server outside the kit's, and the kit's other files, are read.
    files["polyoxide-ws/src/connect.rs"] = BAD
    files["polyoxide-core/src/test_server/mod.rs"] = BAD
    assert found(tree(tmp_path, files)) == [
        ("polyoxide-core/src/test_server/mod.rs", 1, "tokio::spawn"),
        ("polyoxide-ws/src/connect.rs", 1, "tokio::spawn")]


# --- the opt-out ---------------------------------------------------------------


def test_a_line_ending_runtime_gate_with_a_reason_is_exempt_and_nothing_else_is(
        tmp_path: Path) -> None:
    text = (
        "let a = SystemTime::now(); // runtime-gate: the wall clock, never injected\n"
        "let b = SystemTime::now(); // runtime-gate:\n"
        "let c = SystemTime::now(); // live-unwraps: not this gate's opt-out\n"
        "let d = SystemTime::now();\n"
        "// runtime-gate: a comment above does not exempt the line below\n"
        "let e = SystemTime::now();\n"
    )
    root = tree(tmp_path, {CORE: text})
    assert found(root) == [(CORE, n, "SystemTime::now(") for n in (2, 3, 4, 6)]
    assert [n for _, n, _ in found(root, opt_out=False)] == [1, 2, 3, 4, 6]


# --- the command ---------------------------------------------------------------


def test_main_exits_zero_and_prints_nothing_on_a_clean_tree(tmp_path: Path, capsys) -> None:
    tree(tmp_path, {CORE: "fn f() {}\n"})
    assert gate.main([], root=tmp_path) == 0
    captured = capsys.readouterr()
    assert (captured.out, captured.err) == ("", "")


def test_main_exits_one_and_prints_each_hit_as_path_line_banned_path(
        tmp_path: Path, capsys) -> None:
    tree(tmp_path, {CORE: "fn f() {}\nfn g() { tokio::spawn(work()); }\n"})
    assert gate.main([], root=tmp_path) == 1
    captured = capsys.readouterr()
    assert captured.out == "polyoxide-core/src/x.rs:2: tokio::spawn\n"
    assert "1 hit(s)" in captured.err


# --- the real tree -------------------------------------------------------------


def test_the_real_tree_is_clean() -> None:
    assert found(REPO) == []


def test_the_real_tree_is_read_in_every_scanned_crate() -> None:
    crates = {path.relative_to(REPO).parts[0] for path in gate.source_files(REPO)}
    assert crates == set(gate.CRATES)


def test_the_real_trees_wall_clock_reads_are_exempt_by_their_opt_outs() -> None:
    unexempted = {(path, banned) for path, _, banned in found(REPO, opt_out=False)}
    assert {("polyoxide-venue/src/time.rs", "SystemTime::now("),
            ("polyoxide-polymarket/src/shared/auth.rs", "SystemTime::now(")} <= unexempted
