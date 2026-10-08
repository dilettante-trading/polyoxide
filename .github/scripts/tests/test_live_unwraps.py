"""`scripts/live_unwraps.py`: live tests may only lose unwraps, and the
nightly classifier may not gain a regex (AD-14).

The first test runs the check on the real tree, which is what fails CI when a
PR adds an unwrap to a live test or a regex to the classifier. The rest build
small trees to prove each way the check can fail.
"""

from __future__ import annotations

import importlib.util
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
SCRIPT = REPO / "scripts" / "live_unwraps.py"


def _load():
    """`scripts/live_unwraps.py`, which lives outside this uv project."""
    if "live_unwraps" in sys.modules:
        return sys.modules["live_unwraps"]
    spec = importlib.util.spec_from_file_location("live_unwraps", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


live_unwraps = _load()


def counted(source: str) -> int:
    return live_unwraps.count(source)[0]


# --- the real tree -------------------------------------------------------------


def test_the_real_tree_matches_its_baseline() -> None:
    result = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "none added" in result.stdout


def test_the_baseline_freezes_every_regex_table_the_classifier_has() -> None:
    frozen = live_unwraps.read_baseline(live_unwraps.BASELINE)["classifier"]
    assert {"AUTH_GATED_RE", "ENVIRONMENTAL_RE", "TRANSIENT_RES", "TAG_LINE", "PANIC_REPORT"} <= set(frozen)
    assert len(frozen["TRANSIENT_RES"]) >= 30


def test_the_real_tree_counts_a_shared_module_once() -> None:
    """binance's two live targets both declare `mod common;`."""
    unwraps, _ = live_unwraps.counts(REPO)
    assert unwraps["polyoxide-binance/tests/common/mod.rs"] >= 1
    files = [f.relative_to(REPO).as_posix() for f in live_unwraps.live_files(REPO)]
    assert files.count("polyoxide-binance/tests/common/mod.rs") == 1


# --- counting --------------------------------------------------------------------


def test_every_unwrap_and_expect_counts() -> None:
    source = """
let a = client.markets().send().await.unwrap();
let b = client
    .markets()
    .send()
    .await
    .expect("markets");
let c = x.unwrap().y.expect(&format!("{}", 1)).z . unwrap ( );
"""
    assert counted(source) == 5


def test_an_unwrap_named_as_a_path_counts() -> None:
    source = """
let a: Vec<_> = results.into_iter().map(Result::unwrap).collect();
let b = Option::expect(maybe, "present");
let c = items.map(Result::expect);
let d = items.map(Option::unwrap);
let e = items.map(Result::unwrap_or_default);
let f = items.map(Option::unwrap_or);
"""
    assert counted(source) == 4


def test_lookalikes_do_not_count() -> None:
    source = """
let a = x.unwrap_or(1);
let b = x.unwrap_or_else(|| 1);
let c = x.unwrap_err();
let d = x.expect_err("must fail");
let e = x.unwrap_or_default();
fn unwrap() {}
"""
    assert counted(source) == 0


def test_comments_and_literals_do_not_count() -> None:
    source = r'''
//! Call `.unwrap()` in prose, or `.expect("x")`.
/// let a = x.unwrap();
// let b = x.unwrap();
/* let c = x.unwrap();
   /* nested */ let d = x.expect("still a comment");
*/
let e = "x.unwrap() inside a string";
let f = r#"x.expect("inside a raw string")"#;
let g = br"x.unwrap()";
let h = "escaped \" quote then x.unwrap()";
let i = '"'; let j = '\''; let k = x.unwrap(); // a char literal does not open a string
let url = "https://example.test"; let l = y.unwrap(); // `//` in a string opens no comment
fn f<'a>(s: &'a str) -> &'a str { s.trim().expect_none_here() }
'''
    assert counted(source) == 2


def test_a_line_continued_inside_a_string_still_ends_a_line() -> None:
    source = 'let a = "one \\\n two"; let b = x.unwrap();\nlet c = y.unwrap(); // live-unwraps: a constant\n'
    lines = live_unwraps.split_lines(source)
    assert len(lines) == source.count("\n") + 1
    assert live_unwraps.count(source) == (1, 1)


@pytest.mark.parametrize("line,split", [
    ('let a: Url = "https://x.test".parse().unwrap(); // live-unwraps: a constant', (0, 1)),
    ("let a = x.unwrap(); let b = y.unwrap(); // live-unwraps: both parse constants", (0, 2)),
    ("let a = x.unwrap(); //live-unwraps: no space is fine", (0, 1)),
    ("let a = x.unwrap(); // live-unwraps:", (1, 0)),
    ("let a = x.unwrap(); // live-unwraps:    ", (1, 0)),
    ("let a = x.unwrap(); // see live-unwraps: elsewhere", (1, 0)),
    ("let a = x.unwrap(); /// live-unwraps: a doc comment", (1, 0)),
    ("let a = x.unwrap(); /* live-unwraps: a block comment */", (1, 0)),
    ("// live-unwraps: on a line of its own\nlet a = x.unwrap();", (1, 0)),
])
def test_only_a_trailing_comment_with_a_reason_opts_a_line_out(line: str, split: tuple[int, int]) -> None:
    assert live_unwraps.count(line) == split


# --- trees -------------------------------------------------------------------------


def _tree(root: Path, files: dict[str, str]) -> Path:
    """A fake workspace at `root` with `files`, plus the real classifier, and a
    baseline matching both."""
    for path, text in files.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text)
    classifier = root / "classify_failures.py"
    shutil.copyfile(live_unwraps.CLASSIFIER, classifier)
    _rebaseline(root)
    return root


def _rebaseline(root: Path) -> None:
    """Writes the tree's own counts and regexes as its baseline, as a hand edit would."""
    live_unwraps.write_baseline(root / "baseline.json", *live_unwraps.counts(root),
                                live_unwraps.classifier_patterns(root / "classify_failures.py"))


def _check(root: Path) -> list[str]:
    return live_unwraps.check(root, root / "baseline.json", root / "classify_failures.py")


def _lower(root: Path) -> list[str]:
    return live_unwraps.lower(root, root / "baseline.json", root / "classify_failures.py")


LIVE = "mod common;\n\n#[tokio::test]\n#[ignore]\nasync fn live_x() {\n    x().unwrap();\n    y().expect(\"y\");\n}\n"
COMMON = "pub fn client() -> Client {\n    Client::new().unwrap()\n}\n"
OPTED_OUT = '    let u: Url = "https://x.test".parse().unwrap(); // live-unwraps: a constant\n'


@pytest.fixture
def tree(tmp_path: Path) -> Path:
    return _tree(tmp_path, {
        "polyoxide-x/tests/live_api.rs": LIVE,
        "polyoxide-x/tests/common/mod.rs": COMMON,
        "polyoxide-x/tests/mock_api.rs": "fn f() { x().unwrap(); }\n",
    })


def _add_to_live(tree: Path, line: str) -> None:
    live = tree / "polyoxide-x/tests/live_api.rs"
    live.write_text(live.read_text().replace("    y()", line + "    y()"))


def test_a_tree_matching_its_baseline_passes(tree: Path) -> None:
    assert _check(tree) == []
    baseline = json.loads((tree / "baseline.json").read_text())
    assert baseline["unwraps"] == {
        "polyoxide-x/tests/common/mod.rs": 1,
        "polyoxide-x/tests/live_api.rs": 2,
    }, "a mock suite is not a live one"
    assert baseline["opted_out"] == {}


def test_an_added_unwrap_fails(tree: Path) -> None:
    _add_to_live(tree, "    z().unwrap();\n")
    problems = _check(tree)
    assert len(problems) == 1
    assert "polyoxide-x/tests/live_api.rs has 3 unwraps, 1 more than its baseline of 2" in problems[0]
    assert "or_fail" in problems[0]


def test_an_added_unwrap_in_a_shared_module_fails(tree: Path) -> None:
    common = tree / "polyoxide-x/tests/common/mod.rs"
    common.write_text(common.read_text() + "pub fn other() { x().expect(\"x\"); }\n")
    assert [p.split(",")[0] for p in _check(tree)] == [
        "polyoxide-x/tests/common/mod.rs has 2 unwraps"]


def test_an_added_opt_out_fails_and_cannot_be_lowered(tree: Path) -> None:
    _add_to_live(tree, OPTED_OUT)
    problems = _check(tree)
    assert len(problems) == 1
    assert "polyoxide-x/tests/live_api.rs opts 1 unwrap(s) out with `// live-unwraps:`" in problems[0]
    assert _lower(tree) == problems
    # Accepted only when the baseline is raised by hand, in a diff a reviewer sees.
    _rebaseline(tree)
    assert _check(tree) == []
    assert json.loads((tree / "baseline.json").read_text())["opted_out"] == {
        "polyoxide-x/tests/live_api.rs": 1}


def test_a_second_opt_out_in_a_file_that_has_one_fails(tree: Path) -> None:
    _add_to_live(tree, OPTED_OUT)
    _rebaseline(tree)
    _add_to_live(tree, OPTED_OUT)
    problems = _check(tree)
    assert len(problems) == 1 and "opts 2 unwrap(s) out" in problems[0], problems
    assert _lower(tree) == problems


def test_a_removed_opt_out_fails_until_the_baseline_is_lowered(tree: Path) -> None:
    _add_to_live(tree, OPTED_OUT)
    _rebaseline(tree)
    live = tree / "polyoxide-x/tests/live_api.rs"
    live.write_text(live.read_text().replace(OPTED_OUT, ""))
    problems = _check(tree)
    assert len(problems) == 1
    assert "has 0 opted-out unwraps, below its `opted_out` baseline of 1: remove its entry" in problems[0]
    assert _lower(tree) == []
    assert _check(tree) == []


def test_a_removed_unwrap_fails_until_the_baseline_is_lowered(tree: Path) -> None:
    live = tree / "polyoxide-x/tests/live_api.rs"
    live.write_text(live.read_text().replace("    x().unwrap();\n", "    x().or_fail(\"x\");\n"))
    problems = _check(tree)
    assert len(problems) == 1
    assert "has 1 unwraps, below its `unwraps` baseline of 2: lower its entry to 1" in problems[0]

    baseline = json.loads((tree / "baseline.json").read_text())
    baseline["unwraps"]["polyoxide-x/tests/live_api.rs"] = 1
    (tree / "baseline.json").write_text(json.dumps(baseline))
    assert _check(tree) == []


def test_a_file_at_zero_must_leave_the_baseline(tree: Path) -> None:
    (tree / "polyoxide-x/tests/common/mod.rs").write_text("pub fn client() -> Client { Client::new() }\n")
    problems = _check(tree)
    assert len(problems) == 1
    assert "common/mod.rs has 0 unwraps, below its `unwraps` baseline of 1: remove its entry" in problems[0]


def test_a_new_live_file_fails(tree: Path) -> None:
    (tree / "polyoxide-y/tests").mkdir(parents=True)
    (tree / "polyoxide-y/tests/live_ws.rs").write_text("fn f() { x().unwrap(); }\n")
    problems = _check(tree)
    assert len(problems) == 1
    assert problems[0].startswith("polyoxide-y/tests/live_ws.rs has 1 unwrap(s) and no baseline entry")


def test_a_new_module_of_a_live_file_fails(tree: Path) -> None:
    live = tree / "polyoxide-x/tests/live_api.rs"
    live.write_text("mod helpers;\n" + live.read_text())
    (tree / "polyoxide-x/tests/helpers.rs").write_text("pub fn h() { x().unwrap(); }\n")
    problems = _check(tree)
    assert len(problems) == 1
    assert problems[0].startswith("polyoxide-x/tests/helpers.rs has 1 unwrap(s) and no baseline entry")


def test_a_deleted_live_file_must_leave_the_baseline(tree: Path) -> None:
    (tree / "polyoxide-x/tests/live_api.rs").unlink()
    problems = _check(tree)
    assert [p.split(":")[0] for p in problems] == [
        "polyoxide-x/tests/common/mod.rs has 0 unwraps, below its `unwraps` baseline of 1",
        "polyoxide-x/tests/live_api.rs has 0 unwraps, below its `unwraps` baseline of 2",
    ]


# --- the classifier's regexes ----------------------------------------------------------


def _edit_classifier(tree: Path, old: str, new: str) -> None:
    path = tree / "classify_failures.py"
    text = path.read_text()
    assert old in text
    path.write_text(text.replace(old, new, 1))


TRANSIENT_TABLE = "TRANSIENT_RES: list[re.Pattern[str]] = [\n"
CLASSIFY_BODY = '    """\n    if any(TAGS.get(tag'


def test_the_regexes_are_read_from_the_source_alone(tree: Path) -> None:
    """Reading the source finds what running it would, without running it."""
    module = importlib.util.module_from_spec(
        importlib.util.spec_from_file_location("_classifier_run", live_unwraps.CLASSIFIER))
    sys.modules["_classifier_run"] = module
    try:
        module.__spec__.loader.exec_module(module)
    finally:
        del sys.modules["_classifier_run"]
    built = {name: [live_unwraps.spell(p.pattern, p.flags) for p in (v if isinstance(v, list) else [v])]
             for name, v in vars(module).items()
             if isinstance(v, re.Pattern) or (isinstance(v, list) and v and isinstance(v[0], re.Pattern))}
    assert live_unwraps.classifier_patterns(live_unwraps.CLASSIFIER) == built


def test_a_new_transient_regex_fails(tree: Path) -> None:
    _edit_classifier(tree, TRANSIENT_TABLE, TRANSIENT_TABLE + "    re.compile(r\"\\bflaky\\b\"),\n")
    problems = _check(tree)
    assert len(problems) == 1
    assert "TRANSIENT_RES gained `\\bflaky\\b`" in problems[0]
    assert "No regex may be added" in problems[0]


def test_an_inline_search_in_classify_fails(tree: Path) -> None:
    _edit_classifier(tree, CLASSIFY_BODY,
                     '    """\n    if re.search(r"\\bhiccup\\b", failure_output, re.I):\n'
                     '        return Verdict.TRANSIENT\n    if any(TAGS.get(tag')
    problems = _check(tree)
    assert len(problems) == 1
    assert "classify() gained `(?i)\\bhiccup\\b`" in problems[0]
    assert _lower(tree) == problems


@pytest.mark.parametrize("helper,entry", [
    ('def _extra():\n    return re.compile(r"\\bhiccup\\b")\n', "_extra() gained `\\bhiccup\\b`"),
    ('def _extra(text):\n    return re.sub(r"\\s+", " ", text)\n', "_extra() gained `\\s+`"),
    ('from re import search as find\n', None),
    ('PHRASE = r"\\bhiccup\\b"\nMORE = re.compile(PHRASE)\n', "MORE gained `<not a literal: re.compile(PHRASE)>`"),
    ('def _extra(text):\n    return re.match("x", text, flags=FLAGS)\n',
     "_extra() gained `<not a literal: re.match('x', text, flags=FLAGS)>`"),
])
def test_a_regex_anywhere_in_the_module_is_frozen(tree: Path, helper: str, entry: str | None) -> None:
    _edit_classifier(tree, "\n\ndef classify(", f"\n\n{helper}\n\ndef classify(")
    problems = _check(tree)
    if entry is None:
        # An import alone builds nothing; a call through it is caught below.
        assert problems == []
    else:
        assert len(problems) == 1 and entry in problems[0], problems


def test_a_regex_called_through_an_imported_name_is_frozen(tree: Path) -> None:
    _edit_classifier(tree, "\n\ndef classify(",
                     "\n\nfrom re import search as find\n\n\ndef _extra(text):\n"
                     "    return find(r\"\\bhiccup\\b\", text)\n\n\ndef classify(")
    problems = _check(tree)
    assert len(problems) == 1 and "_extra() gained `\\bhiccup\\b`" in problems[0], problems


def test_a_non_regex_item_in_a_frozen_table_fails_and_cannot_be_lowered(tree: Path) -> None:
    _edit_classifier(tree, TRANSIENT_TABLE, TRANSIENT_TABLE + "    \"hiccup\",\n")
    problems = _check(tree)
    assert len(problems) == 1
    assert "TRANSIENT_RES gained `<not a regex: 'hiccup'>`" in problems[0]
    assert _lower(tree) == problems


def test_a_widened_regex_fails(tree: Path) -> None:
    """Making a pattern case-insensitive widens it as surely as a new one."""
    _edit_classifier(tree, 're.compile(r"\\bRateLimit\\("),', 're.compile(r"\\bRateLimit\\(", re.IGNORECASE),')
    problems = _check(tree)
    assert any("gained `(?i)\\bRateLimit\\(`" in p for p in problems), problems
    assert any("no longer holds the regex `\\bRateLimit\\(`" in p for p in problems), problems
    assert len(_lower(tree)) == 1


def test_a_new_regex_table_fails(tree: Path) -> None:
    _edit_classifier(tree, "\n\ndef classify(", "\n\nMORE_RES = (re.compile(r\"\\bhiccup\\b\"),)\n\n\ndef classify(")
    problems = _check(tree)
    assert len(problems) == 1
    assert "MORE_RES gained `\\bhiccup\\b`" in problems[0]


def test_a_removed_regex_fails_until_the_baseline_drops_it(tree: Path) -> None:
    _edit_classifier(tree, '    re.compile(r"\\bRateLimit\\("),\n', "")
    problems = _check(tree)
    assert len(problems) == 1
    assert "TRANSIENT_RES no longer holds the regex `\\bRateLimit\\(`" in problems[0]
    assert _lower(tree) == []
    assert _check(tree) == []


# --- lowering ---------------------------------------------------------------------------


def test_lower_writes_counts_that_fell(tree: Path) -> None:
    live = tree / "polyoxide-x/tests/live_api.rs"
    live.write_text(live.read_text().replace("    x().unwrap();\n", ""))
    (tree / "polyoxide-x/tests/common/mod.rs").write_text("pub fn client() {}\n")
    assert _lower(tree) == []
    assert json.loads((tree / "baseline.json").read_text())["unwraps"] == {
        "polyoxide-x/tests/live_api.rs": 1}
    assert _check(tree) == []


@pytest.mark.parametrize("change", ["rose", "new file", "new regex", "new opt-out"])
def test_lower_refuses_anything_that_grew(tree: Path, change: str) -> None:
    if change == "rose":
        _add_to_live(tree, "    z().unwrap();\n")
    elif change == "new file":
        (tree / "polyoxide-x/tests/live_ws.rs").write_text("fn f() { x().unwrap(); }\n")
    elif change == "new regex":
        _edit_classifier(tree, TRANSIENT_TABLE, TRANSIENT_TABLE + "    re.compile(r\"x\"),\n")
    else:
        _add_to_live(tree, OPTED_OUT)
    before = (tree / "baseline.json").read_text()
    refused = _lower(tree)
    assert len(refused) == 1, refused
    assert (tree / "baseline.json").read_text() == before


def test_spell_writes_flags_inline() -> None:
    assert live_unwraps.spell(r"a\b") == r"a\b"
    assert live_unwraps.spell("a", re.IGNORECASE | re.MULTILINE) == "(?im)a"
