"""Unit tests for scripts/test_body_diff.py.

The normaliser and the item reader run on synthetic source text, with rename rows
built here, so nothing needs scripts/api_removals.py but the one command-line test.
The comparison runs on synthetic git repositories under `tmp_path`, each a baseline
commit tagged `v0` and a checkout edited on top, with git's own configuration kept
out. The tests of the real tree read ci/test-body-diff.toml and divergences.md.
"""

from __future__ import annotations

import importlib.util
import io
import subprocess
import sys
from collections import namedtuple
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]


def _load(name: str):
    """A module from `scripts/`, which lives outside this uv project."""
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


body_diff = _load("test_body_diff")
BodyDiffError = body_diff.BodyDiffError
Config = body_diff.Config
Normaliser = body_diff.Normaliser

# A manifest row, shaped as api_removals.read_manifest returns them.
Row = namedtuple("Row", "old new reason")
CRATE_ROW = Row("polyoxide_x", "polyoxide_y::x", None)
DRIFT_ROWS = frozenset({"R6"})


def normal(text: str, **options) -> list[str]:
    return body_diff.normalise(text, Normaliser(**options))


def same(old: str, new: str, old_options: dict | None = None,
         new_options: dict | None = None) -> bool:
    return normal(old, **(old_options or {})) == normal(new, **(new_options or {}))


# --- normalising -------------------------------------------------------------


def test_layout_does_not_count_and_a_rewrapped_call_is_the_same() -> None:
    old = "fn t() { assert_eq!(trades[0].side, polyoxide_x::TradeSide::Buy); }"
    new = """fn t() {
        assert_eq!(
            trades[0].side,
            polyoxide_x::TradeSide::Buy,
        );
    }"""
    assert same(old, new)


def test_a_generic_list_rewrapped_with_trailing_commas_is_the_same() -> None:
    # rustfmt re-wraps a type under a longer path with a comma before each `>`.
    old = """type Sent = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), polyoxide_x::E>> + Send>,
    >;"""
    new = """type Sent = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), polyoxide_y::x::E>>
                + Send,
        >,
    >;"""
    assert same(old, new, {"rows": (CRATE_ROW,)})
    assert normal(new) == ["type Sent = std :: pin :: Pin < Box < dyn std :: future :: Future < "
                           "Output = Result < ( ) , polyoxide_y :: x :: E > > + Send > > ;"]
    # A comma between two arguments stays.
    assert not same("type T = A<B, C>;", "type T = A<B C>;")


def test_braces_rustfmt_adds_around_a_lone_arm_or_closure_body_do_not_count() -> None:
    old = "fn t() { match x { A => f(a, b), B => g() } v.map(|x| h(x)); }"
    new = """fn t() {
        match x {
            A => {
                f(a, b)
            }
            B => g(),
        }
        v.map(|x| {
            h(x)
        });
    }"""
    assert same(old, new)


def test_a_block_that_holds_a_statement_keeps_its_braces() -> None:
    assert not same("fn t() { match x { A => { f(); } } }", "fn t() { match x { A => f(), } }")


def test_string_contents_keep_their_spacing() -> None:
    assert not same('fn t() { f("a b"); }', 'fn t() { f("a  b"); }')


def test_a_changed_token_counts() -> None:
    assert not same("fn t() { assert_eq!(x, 1); }", "fn t() { assert_eq!(x, 2); }")


def test_comments_doc_attributes_and_use_statements_go_and_other_attributes_stay() -> None:
    old = """/// Says what it does.
    #[doc = "more"]
    #[doc(hidden)]
    #[tokio::test]
    async fn t() {
        use super::*; // a comment
        pub(crate) use polyoxide_x::{A, B};
        /* a block comment */ f();
    }"""
    assert normal(old) == ["# [ doc ( hidden ) ] # [ tokio :: test ] async fn t ( ) {", "f ( ) ;",
                           "}"]
    assert not same("#[test] fn t() {}", "#[test] #[ignore] fn t() {}")


def test_crate_names_the_library_in_src_and_moves_with_its_module() -> None:
    old = "fn t() { crate::types::Side::Buy; }"
    new = "fn t() { crate::x::types::Side::Buy; }"
    assert same(old, new, {"library": "polyoxide_x", "rows": (CRATE_ROW,)},
                {"library": "polyoxide_y"})
    # A test target's `crate::` is its own crate, so it stays.
    assert normal("fn t() { crate::common::f(); }") == ["fn t ( ) {", "crate :: common :: f ( ) ;",
                                                          "}"]


def test_dollar_crate_pub_crate_and_a_method_are_not_paths() -> None:
    text = "pub(crate) fn t() { $crate::f(); x.polyoxide_x(); }"
    assert normal(text, library="lib", rows=(CRATE_ROW,)) == [
        "pub ( crate ) fn t ( ) {", "$ crate :: f ( ) ;", "x . polyoxide_x ( ) ;", "}"]


def test_the_longest_row_wins_and_its_members_and_bare_names_follow() -> None:
    rows = (Row("polyoxide_data", "polyoxide_y::data", None),
            Row("polyoxide_data::DataApiError", "polyoxide_y::data::DataError", None))
    renames = body_diff.renames_of(rows, "polyoxide_data", frozenset())
    assert renames == {"polyoxide_data": "data", "DataApiError": "DataError"}
    old = """fn t() {
        let e: polyoxide_data::DataApiError = polyoxide_data::DataApiError::V2(v);
        let f = polyoxide_data::types::Trade::default();
        matches!(e, DataApiError::V2(_));
    }"""
    new = """fn t() {
        let e: polyoxide_y::data::DataError = polyoxide_y::data::DataError::V2(v);
        let f = polyoxide_y::data::types::Trade::default();
        matches!(e, DataError::V2(_));
    }"""
    assert same(old, new, {"rows": rows, "renames": renames})


def test_a_rename_applies_only_at_a_paths_head() -> None:
    rows = (Row("polyoxide_x::Client::new", "polyoxide_x::Client::create", None),)
    renames = body_diff.renames_of(rows, "polyoxide_x", frozenset())
    assert renames == {"new": "create"}
    old = "fn t() { let v = Vec::new(); let c = x.new(); new(); }"
    assert normal(old, rows=rows, renames=renames) == [
        "fn t ( ) {", "let v = Vec :: new ( ) ;", "let c = x . new ( ) ;", "create ( ) ;", "}"]


def test_a_one_element_tuple_keeps_its_comma_and_a_call_does_not() -> None:
    assert not same("fn t() { let a = (x,); }", "fn t() { let a = (x); }")
    assert not same("fn t() -> (u8,) { return (1,); }", "fn t() -> (u8) { return (1); }")
    assert same("fn t() { f(\n    x,\n); g::<u8>(y,); }", "fn t() { f(x); g::<u8>(y); }")


def test_a_use_that_starts_no_statement_stays() -> None:
    assert normal("fn t() -> impl Sized + use<'a> { x }") == [
        "fn t ( ) -> impl Sized + use < 'a > {", "x", "}"]
    assert normal("fn t() { #[allow(unused)] use a::b; f(); }") == [
        "fn t ( ) {", "# [ allow ( unused ) ] f ( ) ;", "}"]


def test_a_leading_double_colon_path_takes_its_row() -> None:
    assert same("fn t() { ::polyoxide_x::f(); }", "fn t() { ::polyoxide_y::x::f(); }",
                {"rows": (CRATE_ROW,)})


def test_a_reason_row_leaves_its_path_as_it_was() -> None:
    rows = (CRATE_ROW, Row("polyoxide_x::Gone", None, "removed: never constructed"))
    assert normal("fn t() { polyoxide_x::Gone::new(); }", rows=rows) == [
        "fn t ( ) {", "polyoxide_x :: Gone :: new ( ) ;", "}"]


def test_rows_change_only_the_baseline_side() -> None:
    text = "fn t() { polyoxide_x::f(); }"
    assert not same(text, text, {"rows": (CRATE_ROW,)})


def test_a_rename_applies_only_in_a_file_in_or_naming_the_rows_crate() -> None:
    rows = (Row("polyoxide_clob::ws::Channel", "polyoxide_y::clob::ws::ClobChannel", None),)
    assert body_diff.renames_of(rows, "polyoxide_perps", frozenset({"Channel"})) == {}
    assert body_diff.renames_of(rows, "polyoxide_clob", frozenset()) == {
        "Channel": "ClobChannel"}
    assert body_diff.renames_of(rows, None, frozenset({"polyoxide_clob"})) == {
        "Channel": "ClobChannel"}


def test_two_rows_that_rename_one_identifier_differently_cannot_be_decided() -> None:
    rows = (Row("polyoxide_x::Error", "polyoxide_y::x::XError", None),
            Row("polyoxide_x::ws::Error", "polyoxide_y::x::ws::WsError", None))
    with pytest.raises(BodyDiffError, match="renames `Error`"):
        body_diff.renames_of(rows, "polyoxide_x", frozenset())


def test_an_include_target_compares_by_its_blob() -> None:
    blobs = {"c/tests/fixtures/a.json": "aaa", "c/tests/fixtures/x/a.json": "aaa",
             "c/tests/fixtures/b.json": "bbb"}
    old = {"directory": "c/src", "blob": blobs.get}
    new = {"directory": "c/src/m", "blob": blobs.get}
    assert same('const A: &str = include_str!("../tests/fixtures/a.json");',
                'const A: &str = include_str!(\n    "../../tests/fixtures/x/a.json"\n);', old, new)
    assert not same('const A: &str = include_str!("../tests/fixtures/a.json");',
                    'const A: &str = include_str!("../../tests/fixtures/b.json");', old, new)


def test_literals_replace_exact_contents_on_the_baseline_side() -> None:
    literals = {"ws": "clob-ws"}
    assert same('#[cfg(feature = "ws")] fn t() {}', '#[cfg(feature = "clob-ws")] fn t() {}',
                {"literals": literals})
    assert not same('fn t() { f("wss://x"); }', 'fn t() { f("clob-wss://x"); }',
                    {"literals": literals})


def test_a_literal_scope_matches_at_a_slash() -> None:
    config = Config(literals={"polyoxide-clob": {"ws": "clob-ws"}})
    assert config.literals_for("polyoxide-clob/tests/t.rs") == {"ws": "clob-ws"}
    assert config.literals_for("polyoxide-clob-extra/tests/t.rs") == {}


def test_literals_are_scoped_by_the_longest_prefix() -> None:
    config = Config(literals={"": {"a": "everywhere"}, "polyoxide-clob/": {"ws": "clob-ws"},
                              "polyoxide-clob/tests/": {"ws": "tests-ws"}})
    assert config.literals_for("polyoxide-perps/tests/t.rs") == {"a": "everywhere"}
    assert config.literals_for("polyoxide-clob/src/x.rs") == {"a": "everywhere",
                                                               "ws": "clob-ws"}
    assert config.literals_for("polyoxide-clob/tests/t.rs")["ws"] == "tests-ws"


def test_fixtures_reads_a_call_as_naming_its_scopes_subdirectory() -> None:
    old = {"fixtures": "perps"}
    assert same('fn t() { let f = fixtures!().text("time"); }',
                'fn t() { let f = fixtures!("perps").text("time"); }', old)
    assert same('fn t() { let f = fixtures!("ws").text("bbo"); }',
                'fn t() { let f = fixtures!("perps/ws").text("bbo"); }', old)
    assert same('fn t() { polyoxide_test_support::fixtures![] }',
                'fn t() { polyoxide_test_support::fixtures!["perps"] }', old)
    # A call left naming the old directory differs.
    assert not same('fn t() { fixtures!("ws"); }', 'fn t() { fixtures!("ws"); }', old)


def test_fixtures_leaves_what_is_not_a_fixtures_call() -> None:
    old = {"fixtures": "perps"}
    for text in ('fn t() { fixtures::SOCCER; }', 'fn t() { fixtures(); }',
                 'fn t() { fixtures!(DIR); }', 'fn t() { fixtures!("a", "b"); }',
                 'fn t() { other!("ws"); }'):
        assert same(text, text, old), text


def test_fixtures_applies_before_literals() -> None:
    # perps' `"ws"` gate literal must not rewrite `fixtures!("ws")`, whose
    # argument `[fixtures]` has already moved under the scope's subdirectory.
    old = {"fixtures": "perps", "literals": {"ws": "perps-ws"}}
    assert same('#[cfg(feature = "ws")] fn t() { fixtures!("ws"); }',
                '#[cfg(feature = "perps-ws")] fn t() { fixtures!("perps/ws"); }', old)


def test_fixtures_scopes_by_the_longest_prefix_at_a_slash() -> None:
    config = Config(fixtures={"polyoxide-perps": "perps", "polyoxide-perps/tests/ws": "ws"})
    assert config.fixtures_for("polyoxide-perps/tests/t.rs") == ("polyoxide-perps", "perps")
    assert config.fixtures_for("polyoxide-perps/tests/ws/t.rs") == (
        "polyoxide-perps/tests/ws", "ws")
    assert config.fixtures_for("polyoxide-perps-extra/tests/t.rs") is None


# --- reading items -----------------------------------------------------------


@pytest.mark.parametrize(("attribute", "names_test"), [
    ("cfg(test)", True),
    ("cfg(all(test, feature = \"    \"))", True),
    ("cfg(any(test, feature = \"           \"))", True),
    ("cfg(not(not(test)))", True),
    ("cfg(not(test))", False),
    ("cfg(all(not(test), feature = \"  \"))", False),
    ("cfg(feature = \"    \")", False),
    ("cfg(doctest)", False),
    ("cfg_attr(test, allow(dead_code))", False),
])
def test_a_cfg_names_test_outside_a_not(attribute: str, names_test: bool) -> None:
    assert body_diff.cfg_names_test(attribute) is names_test


SOURCE = """\
//! A module.
#![allow(dead_code)]
use std::fmt;

pub const ANSWER: u32 = 42;
static mut COUNT: u32 = 0;
const _: () = assert!(true);
const _: () = assert!(true);
pub(crate) async unsafe fn helper() -> Result<(), [u8; 3]> { Ok(()) }
const fn constant() {}
extern "C" fn callback() {}
struct S { a: u8 }
impl S { fn method(&self) {} }
macro_rules! cases {
    ($name:ident) => { #[test] fn $name() {} };
}
cases!(one);
agreement! { "A" => u8 }
mod inner {
    #[test]
    fn inner_test() { let s = "}"; let c = '{'; }
}
"""


def test_every_module_level_item_is_keyed_by_module_kind_and_name() -> None:
    parsed = body_diff.scan("c/tests/t.rs", SOURCE)
    keys = [(item.module, item.kind, item.name, item.tests) for item, _ in parsed.items]
    assert keys == [
        ("", "const", "ANSWER", 0), ("", "static", "COUNT", 0), ("", "const", "_#1", 0),
        ("", "const", "_#2", 0), ("", "fn", "helper", 0), ("", "fn", "constant", 0),
        ("", "fn", "callback", 0), ("", "struct", "S", 0), ("", "method", "method", 0),
        ("", "macro", "cases", 1),
        ("", "invocation", "cases!#1", 0),
        ("", "invocation", "agreement!#1", 0), ("inner", "fn", "inner_test", 1),
    ]
    lines = {item.name: item.line for item, _ in parsed.items}
    assert lines["helper"] == 9 and lines["inner_test"] == 21


TYPES = """\
#[derive(Debug, Parser)]
struct Cli { #[arg(long)] flag: bool }
enum Kind { A, B }
union U { a: u8 }
type Alias = u8;
extern crate alloc;
extern "C" { fn abs(x: i32) -> i32; }
"""


def test_type_items_are_keyed_and_compared() -> None:
    parsed = body_diff.scan("c/tests/t.rs", TYPES)
    assert [(item.kind, item.name) for item, _ in parsed.items] == [
        ("struct", "Cli"), ("enum", "Kind"), ("union", "U"), ("type", "Alias"),
        ("extern", "alloc"), ("extern", "block#1")]
    cli = parsed.items[0][0].text
    assert not same(cli, cli.replace("#[arg(long)]", "#[arg(short)]"))
    assert not same(cli, cli.replace("Debug, Parser", "Parser"))


def test_a_file_that_does_not_balance_cannot_be_decided() -> None:
    with pytest.raises(BodyDiffError, match="t.rs:2: `}` closes nothing"):
        body_diff.scan("t.rs", "fn a() {}\n}\n")


def test_an_item_it_cannot_read_cannot_be_decided() -> None:
    with pytest.raises(BodyDiffError, match="cannot read the item"):
        body_diff.scan("t.rs", "let x = 1;\n")


class FakeTree(body_diff.Tree):
    label = "fake"

    def __init__(self, files: dict[str, str]):
        self.files = files

    def paths(self) -> list[str]:
        return sorted(self.files)

    def read(self, path: str) -> str:
        return self.files[path]

    def blob(self, path: str) -> str | None:
        text = self.files.get(path)
        return body_diff.blob_id(text.encode()) if text is not None else None


CRATE = {
    "c/Cargo.toml": """\
[package]
name = "polyoxide-c"
version = "0.0.0"

[[example]]
name = "soak"
test = true

[[example]]
name = "probe"
""",
    "c/src/lib.rs": """\
pub mod a;
#[cfg(test)]
mod b;
#[cfg(not(test))]
mod c;
#[cfg(any(test, feature = "test-server"))]
pub mod fixtures;
pub fn library() {}
#[test]
fn loose_test() {}
#[cfg(test)]
fn loose_helper() {}
""",
    "c/src/a.rs": "pub fn a() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn in_a() {}\n}\n",
    "c/src/b.rs": "fn helper_in_b() {}\nmod more;\n",
    "c/src/b/more.rs": "fn in_more() {}\n",
    "c/src/c.rs": "fn not_test() {}\n",
    "c/src/fixtures.rs": "pub const FRAME: &str = \"{}\";\n",
    "c/src/scripted.rs": "pub fn scripted() {}\n",
    "c/tests/t.rs": ("#[path = \"../src/scripted.rs\"]\nmod scripted;\nmod common;\n"
                     "#[test]\nfn t() {}\n"),
    "c/tests/common/mod.rs": "pub fn shared() {}\n",
    "c/examples/soak.rs": "fn main() {}\n",
    "c/examples/probe.rs": "fn probe_main() {}\n",
}


def test_test_context_covers_tests_test_examples_test_modules_and_test_items() -> None:
    side = body_diff.read_side(FakeTree(CRATE))
    assert sorted((item.file, item.label) for item in side.items.values()
                  if item.kind in body_diff.RUST_KINDS) == [
        ("c/examples/soak.rs", "main"),
        ("c/src/a.rs", "tests::in_a"),
        ("c/src/b.rs", "helper_in_b"),
        ("c/src/b/more.rs", "in_more"),
        ("c/src/fixtures.rs", "FRAME"),
        ("c/src/lib.rs", "loose_helper"),
        ("c/src/lib.rs", "loose_test"),
        ("c/src/scripted.rs", "scripted"),
        ("c/tests/common/mod.rs", "shared"),
        ("c/tests/t.rs", "t"),
    ]
    assert side.owners["c/src/a.rs"].library == "polyoxide_c"


C_MANIFEST = '[package]\nname = "polyoxide-c"\nversion = "0.0.0"\n'


def _texts(files: dict[str, str]) -> dict[str, list[str]]:
    side = body_diff.read_side(FakeTree(files))
    return {item.label: normal(item.text) for item in side.items.values()
            if item.kind in body_diff.RUST_KINDS}


GATED = {
    "c/Cargo.toml": C_MANIFEST,
    "c/src/lib.rs": "#[cfg(test)]\n#[path = \"b.rs\"]\nmod b;\n",
    "c/src/b.rs": "#![cfg(unix)]\n#![allow(dead_code)]\nmod inner {\n    fn t() {}\n}\n",
}


def test_what_gates_an_item_is_compared_with_it() -> None:
    texts = _texts(GATED)
    # The `mod b;` declaration's attributes but its `#[path]`, the file's inner `cfg`
    # but no lint, then the enclosing module's attributes.
    assert texts["inner::t"] == ["# [ cfg ( test ) ] # ! [ cfg ( unix ) ] fn t ( ) {", "}"]
    for path, old, new in [
            ("c/src/lib.rs", "#[cfg(test)]", "#[cfg(all(test, any()))]"),
            ("c/src/b.rs", "#![cfg(unix)]", "#![cfg(windows)]"),
            ("c/src/b.rs", "mod inner", "#[cfg(any())]\nmod inner")]:
        changed = {**GATED, path: GATED[path].replace(old, new, 1)}
        assert _texts(changed)["inner::t"] != texts["inner::t"], (path, new)
    moved = {**GATED, "c/src/lib.rs": GATED["c/src/lib.rs"].replace("b.rs", "./b.rs")}
    assert _texts(moved) == texts


def test_a_file_whose_inner_cfg_names_test_is_a_test_context() -> None:
    files = {"c/Cargo.toml": C_MANIFEST, "c/src/lib.rs": "mod t;\n",
             "c/src/t.rs": "#![cfg(test)]\nfn helper() {}\nmod deeper;\n",
             "c/src/t/deeper.rs": "fn deeper() {}\n"}
    assert sorted(_texts(files)) == ["deeper", "helper"]


def test_a_test_or_bench_target_outside_tests_is_a_test_context() -> None:
    files = {"c/Cargo.toml": C_MANIFEST + (
                 '[[test]]\nname = "suite"\npath = "suites/suite.rs"\n'
                 '[[bench]]\nname = "speed"\npath = "speed/main.rs"\n'),
             "c/suites/suite.rs": "#[test]\nfn t() {}\n",
             "c/speed/main.rs": "fn main() {}\n"}
    assert sorted(_texts(files)) == ["main", "t"]


def test_a_manifest_that_does_not_parse_is_skipped_with_a_warning(capsys) -> None:
    files = {"c/Cargo.toml": "[package\n", "c/tests/t.rs": "#[test]\nfn t() {}\n"}
    assert body_diff.read_side(FakeTree(files)).items == {}
    assert "::warning::fake:c/Cargo.toml is not valid TOML" in capsys.readouterr().err


METHODS = """\
struct Recorder;
impl Recorder {
    const LIMIT: u32 = 3;
    type Unused = ();
    fn holds(&self) -> u32 { Self::LIMIT }
    helper_macro!();
}
impl<T: Clone> polyoxide_x::Throttle<T> for Recorder where T: Send {
    fn acquire(&self) {}
}
pub trait Probe: Send {
    const NAME: &'static str;
    fn required(&self);
    fn provided(&self) -> u8 { 1 }
}
#[cfg(feature = "a")]
impl Recorder { fn gated(&self) {} }
#[cfg(not(feature = "a"))]
impl Recorder { fn gated(&self) {} }
impl Recorder { fn twice(&self) {} }
impl Recorder { fn twice(&self) {} }
"""


def test_methods_are_keyed_by_their_type_trait_impl_or_trait() -> None:
    files = {"c/Cargo.toml": CRATE["c/Cargo.toml"], "c/tests/t.rs": METHODS}
    side = body_diff.read_side(FakeTree(files))
    assert sorted((item.kind, item.name, item.line) for item in side.items.values()) == [
        ("assoc_const", "Probe::NAME", 12), ("assoc_const", "Recorder::LIMIT", 3),
        ("assoc_type", "Recorder::Unused", 4),
        ("method", "<impl polyoxide_x::Throttle<T> for Recorder>::acquire", 9),
        ("method", "Probe::provided", 14), ("method", "Probe::required", 13),
        ("method", 'Recorder::gated #[cfg(feature="a")]', 17),
        ("method", 'Recorder::gated #[cfg(not(feature="a"))]', 19),
        ("method", "Recorder::holds", 5), ("method", "Recorder::twice#1", 20),
        ("method", "Recorder::twice#2", 21), ("struct", "Recorder", 1),
    ]
    acquire = side.items[("c/tests/t.rs", "", "method",
                          "<impl polyoxide_x::Throttle<T> for Recorder>::acquire")]
    # The block's header is compared with each of its methods.
    assert normal(acquire.text) == [
        "impl < T : Clone > polyoxide_x :: Throttle < T > for Recorder where T : Send {",
        "fn acquire ( & self ) {", "}", "}"]


def test_only_a_test_context_impls_methods_count() -> None:
    files = {"c/Cargo.toml": CRATE["c/Cargo.toml"], "c/src/lib.rs": """\
pub struct S;
impl S {
    pub fn library(&self) {}
    #[cfg(test)]
    pub(crate) fn for_tests(&self) {}
}
#[cfg(test)]
impl S { fn test_only(&self) {} }
#[cfg(test)]
mod tests {
    impl super::S { fn in_tests(&self) {} }
}
"""}
    side = body_diff.read_side(FakeTree(files))
    assert sorted(item.label for item in side.items.values()) == [
        "S::for_tests", "S::test_only", "tests::super::S::in_tests"]


def test_two_items_of_one_key_cannot_be_told_apart() -> None:
    files = {"c/Cargo.toml": CRATE["c/Cargo.toml"],
             "c/tests/t.rs": ("#[cfg(feature = \"a\")]\nfn f() {}\n"
                              "#[cfg(not(feature = \"a\"))]\nfn f() {}\n")}
    with pytest.raises(BodyDiffError, match="second fn f"):
        body_diff.read_side(FakeTree(files))


# --- ci/test-body-diff.toml --------------------------------------------------


SHA = "a" * 64


def test_the_config_reads_every_section() -> None:
    config = body_diff.parse_config(f"""
[files]
"a/tests/x.rs" = "b/tests/y.rs"

[literals]
"polyoxide_x" = "polyoxide_y::x"

[literals."a/"]
"ws" = "x-ws"

[fixtures]
"a" = "x"

[paths]
"polyoxide_x::private::Thing" = "polyoxide_y::x::shared::Thing"

[gates]
"b/src/clob/" = "clob"
"b/tests/" = ["clob", "gamma"]
"b/src/shared/" = 'any(feature = "clob", feature = "relay")'

[[drift]]
file = "a/tests/x.rs"
item = "tests::t"
row = "R6"
sha256 = "{SHA}"

[[retired]]
file = "a/tests/x.rs"
item = "u"
kind = "const"
row = "polyoxide_x::Gone"
""")
    assert config.files == {"a/tests/x.rs": "b/tests/y.rs"}
    assert config.paths == {"polyoxide_x::private::Thing": "polyoxide_y::x::shared::Thing"}
    assert config.gates == {"b/src/clob/": ("clob",), "b/tests/": ("clob", "gamma"),
                            "b/src/shared/": ('any(feature = "clob", feature = "relay")',)}
    assert config.literals == {"": {"polyoxide_x": "polyoxide_y::x"}, "a/": {"ws": "x-ws"}}
    assert config.fixtures == {"a": "x"}
    assert config.drift[0].key == ("a/tests/x.rs", "fn", "tests::t")
    assert config.retired[0].key == ("a/tests/x.rs", "const", "u")


@pytest.mark.parametrize(("text", "message"), [
    ("[other]\n", "unknown keys"),
    ("[files]\n\"a\" = 1\n", "must be a table mapping strings"),
    ("[[drift]]\nfile = \"a\"\nitem = \"t\"\nrow = \"R6\"\n", "must give"),
    (f"[[drift]]\nfile = \"a\"\nitem = \"t\"\nrow = \"R6\"\nsha256 = \"{SHA}\"\nkind = \"impl\"\n",
     "kind 'impl'"),
    ("[[drift]]\nfile = \"a\"\nitem = \"t\"\nrow = \"R6\"\nsha256 = \"abc\"\n", "64 lowercase"),
    ("[files\n", "not valid TOML"),
    ("literals = []\n", "`\\[literals\\]` must be a table"),
    ("literals = \"x\"\n", "`\\[literals\\]` must be a table"),
    ("files = 1\n", "must be a table"),
    ("paths = []\n", "must be a table"),
    ("[paths]\n\"a\" = 1\n", "must be a table"),
    ("gates = []\n", "`\\[gates\\]` must be a table"),
    ("fixtures = []\n", "`\\[fixtures\\]` must be a table"),
    ("[fixtures]\n\"a\" = 1\n", "`\\[fixtures\\]` must be a table"),
    ("[fixtures]\n\"a\" = \"\"\n", "no leading or trailing"),
    ("[fixtures]\n\"a\" = \"/x\"\n", "no leading or trailing"),
    ("[fixtures]\n\"a\" = \"x/\"\n", "no leading or trailing"),
    ("[gates]\n\"a/\" = 1\n", "give a feature or a cfg predicate, or an array"),
    ("[gates]\n\"a/\" = []\n", "give a feature or a cfg predicate, or an array"),
    ("[gates]\n\"a/\" = \" \"\n", "give a feature or a cfg predicate, or an array"),
    ("[gates]\n\"a/\" = 'any(feature = \"m\"'\n", "does not read as a cfg predicate"),
    ("[gates]\n\"a/\" = ')any('\n", "does not read as a cfg predicate"),
    ("[gates]\n\"a/\" = '= \"m\"'\n", "does not read as a cfg predicate"),
    (f"[[drift]]\nfile = \"a\"\nitem = \"t\"\nrow = \"R6\"\nsha256 = \"{SHA}\"\n" * 2,
     "an earlier `\\[\\[drift\\]\\]` entry names too"),
    ("[[retired]]\nfile = \"a\"\nitem = \"t\"\nrow = \"x::Y\"\n" * 2,
     "an earlier `\\[\\[retired\\]\\]` entry names too"),
])
def test_a_malformed_config_cannot_be_decided(text: str, message: str) -> None:
    with pytest.raises(BodyDiffError, match=message):
        body_diff.parse_config(text)


def test_drift_rows_are_read_from_the_table() -> None:
    assert body_diff.drift_rows("| R1 | x |\n| R11 | y |\n| D4 | z |\n|R2|\n") == {
        "R1", "R11", "R2"}


# --- comparing two trees -----------------------------------------------------


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout


def write(repo: Path, files: dict[str, str | None]) -> None:
    for path, text in files.items():
        file = repo / path
        if text is None:
            file.unlink()
        else:
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(text)


@pytest.fixture
def repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """An empty repository, with only this test's git configuration."""
    config = tmp_path / "gitconfig"
    config.write_text("[user]\n\tname = t\n\temail = t@t\n[commit]\n\tgpgsign = false\n"
                      "[tag]\n\tgpgsign = false\n[init]\n\tdefaultBranch = main\n")
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", str(config))
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "-q")
    return repo


def baseline(repo: Path, files: dict[str, str]) -> None:
    write(repo, files)
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "baseline")
    git(repo, "tag", "v0")


def edit(repo: Path, files: dict[str, str | None]) -> None:
    write(repo, files)
    git(repo, "add", "-A")


def package(name: str) -> str:
    return f'[package]\nname = "{name}"\nversion = "0.0.0"\n'


X_TESTS = """\
use polyoxide_x::{Client, XError};

fn client() -> Client {
    polyoxide_x::Client::new("http://localhost")
}

#[test]
fn a_request_fails_with_its_error() {
    let err = client().get().unwrap_err();
    assert!(matches!(err, XError::Status(404)));
    assert_eq!(polyoxide_x::types::Side::Buy.as_str(), "BUY");
}

#[test]
fn another() {
    assert_eq!(1 + 1, 2);
}
"""

X_SRC = """\
pub mod types;
pub struct Client;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_names() {
        assert_eq!(crate::types::Side::Buy.as_str(), "BUY");
    }
}
"""

TREE = {
    "Cargo.toml": "[workspace]\nmembers = [\"polyoxide-x\"]\n",
    "polyoxide-x/Cargo.toml": package("polyoxide-x"),
    "polyoxide-x/src/lib.rs": X_SRC,
    "polyoxide-x/src/types.rs": "pub enum Side { Buy }\n",
    "polyoxide-x/tests/mock_api.rs": X_TESTS,
}


def moved(tests: str = X_TESTS, src: str = X_SRC) -> dict[str, str | None]:
    """`TREE`'s crate as module `x` of `polyoxide-y`, with its paths rewritten."""
    return {
        "Cargo.toml": "[workspace]\nmembers = [\"polyoxide-y\"]\n",
        "polyoxide-x/Cargo.toml": None, "polyoxide-x/src/lib.rs": None,
        "polyoxide-x/src/types.rs": None, "polyoxide-x/tests/mock_api.rs": None,
        "polyoxide-y/Cargo.toml": package("polyoxide-y"),
        "polyoxide-y/src/lib.rs": "pub mod x;\n",
        "polyoxide-y/src/x/mod.rs": src.replace("crate::", "crate::x::"),
        "polyoxide-y/src/x/types.rs": "pub enum Side { Buy }\n",
        "polyoxide-y/tests/mock_x.rs": tests.replace("polyoxide_x::", "polyoxide_y::x::"),
    }


def compare(repo: Path, rows=(), config: Config | None = None):
    return body_diff.compare(repo, "v0", tuple(rows), config or Config(), DRIFT_ROWS)


def reported(result, moves: bool = False) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    status = body_diff.report(result, moves, out, err)
    return status, out.getvalue(), err.getvalue()


def test_no_moves_pairs_every_item_with_no_diff(repo: Path) -> None:
    baseline(repo, TREE)
    result = compare(repo)
    status, out, err = reported(result)
    assert (status, err) == (0, "")
    assert len(result.pairs) == 4 and not result.diffs and not result.lost
    assert "polyoxide-x/tests/mock_api.rs: 2 tests, 1 helpers -> 2 tests, 1 helpers" in out
    assert out.splitlines()[-1].startswith(
        "3 tests, 1 helpers, 0 fixtures and 0 target stanzas at v0; 4 paired, 0 differ")


def test_a_crate_moved_under_a_row_compares_equal(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved())
    result = compare(repo, [CRATE_ROW])
    status, out, _ = reported(result)
    assert status == 0 and len(result.pairs) == 4
    assert result.file_pairs["polyoxide-x/tests/mock_api.rs"] == "polyoxide-y/tests/mock_x.rs"
    assert ("polyoxide-x/tests/mock_api.rs -> polyoxide-y/tests/mock_x.rs: "
            "2 tests, 1 helpers -> 2 tests, 1 helpers") in out


def test_a_crate_moved_with_no_row_differs_on_every_path_it_names(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved())
    result = compare(repo)
    assert result.status == 1
    assert sorted(d.old.label for d in result.diffs) == [
        "a_request_fails_with_its_error", "client", "tests::side_names"]


def test_an_item_rename_follows_its_row_over_the_crates(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(X_TESTS.replace("XError", "YError")))
    rows = [CRATE_ROW, Row("polyoxide_x::XError", "polyoxide_y::x::YError", None)]
    assert compare(repo, rows).status == 0
    assert [d.old.label for d in compare(repo, [CRATE_ROW]).diffs] == [
        "a_request_fails_with_its_error"]


def test_a_changed_body_prints_its_diff_and_fails(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(X_TESTS.replace("(1 + 1, 2)", "(1 + 1, 3)")))
    result = compare(repo, [CRATE_ROW])
    status, _, err = reported(result)
    assert status == 1 and len(result.diffs) == 1
    found = result.diffs[0]
    assert found.old.where == "polyoxide-x/tests/mock_api.rs:15 fn another"
    assert f"::error::polyoxide-x/tests/mock_api.rs:15 fn another differs at " \
           f"polyoxide-y/tests/mock_x.rs:15 after normalising (sha256 {found.sha256})" in err
    assert "  -assert_eq ! ( 1 + 1 , 2 ) ;\n  +assert_eq ! ( 1 + 1 , 3 ) ;\n" in err


def drift(sha256: str, row: str = "R6") -> Config:
    return Config(drift=(body_diff.Entry("polyoxide-x/tests/mock_api.rs", "another", "fn", row,
                                         sha256),))


def test_an_acknowledged_diff_passes_and_says_so(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(X_TESTS.replace("(1 + 1, 2)", "(1 + 1, 3)")))
    sha256 = compare(repo, [CRATE_ROW]).diffs[0].sha256
    status, out, err = reported(compare(repo, [CRATE_ROW], drift(sha256)))
    assert (status, err) == (0, "")
    assert (f"Acknowledged as DRIFT R6: polyoxide-x/tests/mock_api.rs:15 fn another -> "
            f"polyoxide-y/tests/mock_x.rs:15 fn another (sha256 {sha256})") in out


def test_an_acknowledgement_naming_no_drift_row_fails(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(X_TESTS.replace("(1 + 1, 2)", "(1 + 1, 3)")))
    sha256 = compare(repo, [CRATE_ROW]).diffs[0].sha256
    result = compare(repo, [CRATE_ROW], drift(sha256, "D7"))
    assert result.status == 1 and len(result.diffs) == 1
    assert result.problems == ["`[[drift]]` for polyoxide-x/tests/mock_api.rs fn another "
                               "names D7, which is not a DRIFT row of "
                               f"{body_diff.DIVERGENCES}"]


def test_a_stale_acknowledgement_fails_and_the_diff_shows(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(X_TESTS.replace("(1 + 1, 2)", "(1 + 1, 3)")))
    result = compare(repo, [CRATE_ROW], drift("b" * 64))
    assert result.status == 1 and len(result.diffs) == 1
    assert "is stale: that item has no diff whose sha256 is bbbb" in result.problems[0]


def test_an_acknowledgement_of_a_body_that_no_longer_differs_is_stale(repo: Path) -> None:
    baseline(repo, TREE)
    result = compare(repo, config=drift("b" * 64))
    assert result.status == 1 and not result.diffs and len(result.problems) == 1


def without_another(tests: str = X_TESTS) -> str:
    return tests[:tests.index("#[test]\nfn another")]


def retired(row: str) -> Config:
    return Config(retired=(body_diff.Entry("polyoxide-x/tests/mock_api.rs", "another", "fn",
                                           row),))


def test_a_lost_test_fails_and_is_named(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(without_another()))
    status, _, err = reported(compare(repo, [CRATE_ROW]))
    assert status == 1
    assert "::error::polyoxide-x/tests/mock_api.rs:15 fn another has no counterpart" in err


def test_a_test_retired_with_a_reason_row_passes(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(without_another()))
    rows = [CRATE_ROW, Row("polyoxide_x::Arithmetic", None, "removed: never shipped (5.4)")]
    status, out, err = reported(compare(repo, rows, retired("polyoxide_x::Arithmetic::add")))
    assert (status, err) == (0, "")
    assert ("Retired with the manifest's `polyoxide_x::Arithmetic::add`: "
            "polyoxide-x/tests/mock_api.rs:15 fn another") in out


def test_a_retirement_citing_no_row_or_a_path_row_fails(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(without_another()))
    missing = compare(repo, [Row("polyoxide_z", None, "gone")], retired("polyoxide_x::Gone"))
    assert missing.status == 1 and "which no manifest row covers" in missing.problems[0]
    path = compare(repo, [CRATE_ROW], retired("polyoxide_x::Gone"))
    assert path.status == 1 and len(path.lost) == 1
    assert "maps to `polyoxide_y::x`" in path.problems[0]


def test_a_retirement_of_a_test_that_is_not_lost_is_stale(repo: Path) -> None:
    baseline(repo, TREE)
    result = compare(repo, [Row("polyoxide_x::Gone", None, "gone")], retired("polyoxide_x::Gone"))
    assert result.status == 1 and "is stale" in result.problems[0]


def test_an_item_left_unpaired_pairs_by_a_unique_name_anywhere(repo: Path) -> None:
    baseline(repo, TREE)
    # Rewritten past 30% similarity, so git pairs nothing.
    filler = "".join(f"#[test]\nfn new_{n}() {{ assert!(f({n})); }}\n" for n in range(40))
    edit(repo, {"polyoxide-x/tests/mock_api.rs": None,
                "polyoxide-x/tests/other.rs": X_TESTS + filler})
    result = compare(repo)
    assert result.status == 0
    assert "polyoxide-x/tests/mock_api.rs" not in body_diff.file_pairs(repo, "v0").values()
    assert len(result.added) == 40


def test_files_pairs_what_a_unique_name_cannot(repo: Path) -> None:
    baseline(repo, TREE)
    filler = "".join(f"#[test]\nfn new_{n}() {{ assert!(f({n})); }}\n" for n in range(40))
    edit(repo, {"polyoxide-x/tests/mock_api.rs": None,
                "polyoxide-x/tests/other.rs": X_TESTS + filler,
                "polyoxide-x/tests/third.rs": "fn client() {}\n"})
    assert [item.label for item in compare(repo).lost] == ["client"]
    config = Config(files={"polyoxide-x/tests/mock_api.rs": "polyoxide-x/tests/other.rs"})
    assert compare(repo, config=config).status == 0
    stale = Config(files={"polyoxide-x/tests/gone.rs": "polyoxide-x/tests/other.rs"})
    assert "polyoxide-x/tests/gone.rs is not in v0" in compare(repo, config=stale).problems[0]


FAKE = """\
use polyoxide_x::Throttle;

struct Recorder;

impl polyoxide_x::Throttle for Recorder {
    fn acquire(&self) -> u32 {
        1
    }
}

#[test]
fn throttles() {
    assert_eq!(Recorder.acquire(), 1);
}
"""


def test_a_fake_impl_moves_under_its_row_and_a_changed_body_is_one_diff(repo: Path) -> None:
    baseline(repo, {**TREE, "polyoxide-x/tests/fake.rs": FAKE})
    moved_fake = FAKE.replace("polyoxide_x::", "polyoxide_y::x::")
    edit(repo, {**moved(), "polyoxide-x/tests/fake.rs": None,
                "polyoxide-y/tests/fake_x.rs": moved_fake})
    result = compare(repo, [CRATE_ROW])
    assert result.status == 0 and len(result.pairs) == 7
    edit(repo, {"polyoxide-y/tests/fake_x.rs": moved_fake.replace("        1\n", "        2\n")})
    result = compare(repo, [CRATE_ROW])
    assert [d.old.where for d in result.diffs] == [
        "polyoxide-x/tests/fake.rs:6 method <impl polyoxide_y::x::Throttle for Recorder>::acquire"]
    entry = body_diff.Entry("polyoxide-x/tests/fake.rs",
                            "<impl polyoxide_y::x::Throttle for Recorder>::acquire", "method",
                            "R6", result.diffs[0].sha256)
    assert compare(repo, [CRATE_ROW], Config(drift=(entry,))).status == 0


def test_moves_prints_each_moved_item_for_the_ledger(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved("\n\n" + X_TESTS))
    _, out, _ = reported(compare(repo, [CRATE_ROW]), moves=True)
    assert ("polyoxide-x/tests/mock_api.rs:15 another -> polyoxide-y/tests/mock_x.rs:17"
            in out.splitlines())
    assert ("polyoxide-x/src/lib.rs:8 side_names -> polyoxide-y/src/x/mod.rs:8"
            in out.splitlines())


A_AGREEMENT = """\
#[test]
fn every_fixture_decodes() {
    let text = polyoxide_test_support::fixtures!().text("time");
    let ws = polyoxide_test_support::fixtures!("ws").text("bbo");
    assert_eq!(text.len() + ws.len(), 2);
}
"""


def test_a_fixtures_call_moved_under_a_module_subdirectory_compares_equal(repo: Path) -> None:
    baseline(repo, {**TREE, "polyoxide-x/tests/agreement.rs": A_AGREEMENT})
    edit(repo, {**moved(), "polyoxide-x/tests/agreement.rs": None,
                "polyoxide-y/tests/agreement_x.rs": A_AGREEMENT.replace(
                    "fixtures!()", 'fixtures!("x")').replace('fixtures!("ws")',
                                                            'fixtures!("x/ws")')})
    assert compare(repo, [CRATE_ROW]).diffs != []
    result = compare(repo, [CRATE_ROW], Config(fixtures={"polyoxide-x": "x"}))
    assert (result.status, result.problems) == (0, [])
    # An entry whose scope holds no call is stale.
    stale = compare(repo, [CRATE_ROW], Config(fixtures={"polyoxide-x": "x",
                                                        "polyoxide-z": "z"}))
    assert stale.problems == ["`[fixtures]` reads the `fixtures!` calls under polyoxide-z "
                              "as naming 'z', but no baseline call there was rewritten"]


def test_a_fixture_that_moves_with_its_content_compares_equal(repo: Path) -> None:
    files = dict(TREE)
    files["polyoxide-x/src/types.rs"] = (
        "#[cfg(test)]\nmod tests {\n    const FRAME: &str = "
        "include_str!(\"../tests/fixtures/frame.json\");\n}\n")
    files["polyoxide-x/tests/fixtures/frame.json"] = "{}\n"
    baseline(repo, files)
    edit(repo, {"polyoxide-x/src/types.rs": files["polyoxide-x/src/types.rs"].replace(
                    "../tests/fixtures/frame.json", "../tests/fixtures/x/frame.json"),
                "polyoxide-x/tests/fixtures/frame.json": None,
                "polyoxide-x/tests/fixtures/x/frame.json": "{}\n"})
    assert compare(repo).status == 0
    edit(repo, {"polyoxide-x/tests/fixtures/x/frame.json": "{\"a\": 1}\n"})
    assert sorted(d.old.where for d in compare(repo).diffs) == [
        "polyoxide-x/src/types.rs:3 const tests::FRAME",
        "polyoxide-x/tests/fixtures/frame.json fixture frame.json"]


def test_a_changed_fixture_is_a_diff_a_drift_entry_can_acknowledge(repo: Path) -> None:
    files = {**TREE, "polyoxide-x/tests/fixtures/book.json": "{}\n",
             "polyoxide-x/tests/fixtures/PROVENANCE.md": "Captured once.\n"}
    baseline(repo, files)
    edit(repo, {"polyoxide-x/tests/fixtures/book.json": "{\"a\": 1}\n",
                "polyoxide-x/tests/fixtures/PROVENANCE.md": "Captured twice.\n"})
    result = compare(repo)
    assert [d.old.where for d in result.diffs] == [
        "polyoxide-x/tests/fixtures/book.json fixture book.json"]
    entry = body_diff.Entry("polyoxide-x/tests/fixtures/book.json", "book.json", "fixture",
                            "R6", result.diffs[0].sha256)
    assert compare(repo, config=Config(drift=(entry,))).status == 0


def test_a_lost_fixture_is_lost_unless_retired(repo: Path) -> None:
    baseline(repo, {**TREE, "polyoxide-x/tests/fixtures/book.json": "{}\n"})
    edit(repo, {"polyoxide-x/tests/fixtures/book.json": None})
    assert [item.where for item in compare(repo).lost] == [
        "polyoxide-x/tests/fixtures/book.json fixture book.json"]
    entry = body_diff.Entry("polyoxide-x/tests/fixtures/book.json", "book.json", "fixture",
                            "polyoxide_x::Book")
    rows = [Row("polyoxide_x::Book", None, "removed: never shipped")]
    assert compare(repo, rows, Config(retired=(entry,))).status == 0


STANZA = '[[test]]\nname = "live_api"\npath = "tests/live_api.rs"\nrequired-features = ["ws"]\n'


def test_a_test_stanza_compares_every_field_but_its_name_and_path(repo: Path) -> None:
    files = {**TREE, "polyoxide-x/Cargo.toml": package("polyoxide-x") + STANZA,
             "polyoxide-x/tests/live_api.rs": "#[test]\nfn live() {}\n"}
    baseline(repo, files)
    edit(repo, {"polyoxide-x/Cargo.toml": package("polyoxide-x") + STANZA.replace(
                    "live_api", "live_x"),
                "polyoxide-x/tests/live_api.rs": None,
                "polyoxide-x/tests/live_x.rs": "#[test]\nfn live() {}\n"})
    assert compare(repo).status == 0
    edit(repo, {"polyoxide-x/Cargo.toml": package("polyoxide-x") + STANZA.replace(
                    "live_api", "live_x").replace('["ws"]', '["x-ws"]')})
    result = compare(repo)
    assert [d.old.where for d in result.diffs] == [
        "polyoxide-x/tests/live_api.rs target [[test]]"]
    features = (Row("polyoxide-x/ws", "polyoxide-x/x-ws", None),)
    assert body_diff.compare(repo, "v0", (), Config(), DRIFT_ROWS,
                             features=features).status == 0


def test_a_path_entry_maps_a_private_path_and_wins_over_a_row(repo: Path) -> None:
    tests = X_TESTS.replace("polyoxide_x::types::Side::Buy", "polyoxide_x::types::Side::Sell")
    baseline(repo, {**TREE, "polyoxide-x/tests/mock_api.rs": tests})
    edit(repo, moved(tests.replace("polyoxide_x::types", "polyoxide_x::shared"),
                     X_SRC.replace("crate::types", "crate::shared")))
    assert sorted(d.old.label for d in compare(repo, [CRATE_ROW]).diffs) == [
        "a_request_fails_with_its_error", "tests::side_names"]
    paths = Config(paths={"polyoxide_x::types": "polyoxide_y::x::shared"})
    assert compare(repo, [CRATE_ROW], paths).status == 0
    rival = [CRATE_ROW, Row("polyoxide_x::types", "polyoxide_y::x::types", None)]
    assert compare(repo, rival, paths).status == 0


def test_an_entry_that_replaces_or_pairs_nothing_is_stale(repo: Path) -> None:
    baseline(repo, {**TREE, "polyoxide-x/README.txt": "x\n"})
    edit(repo, {"polyoxide-x/README2.txt": "y\n"})
    config = Config(literals={"": {"never": "x"}, "polyoxide-x/": {"BUY": "BUY"}},
                    files={"polyoxide-x/README.txt": "polyoxide-x/README2.txt"},
                    paths={"polyoxide_z::Gone": "polyoxide_y::Gone"})
    problems = compare(repo, config=config).problems
    assert problems == [
        "`[files]` pairs polyoxide-x/README.txt with polyoxide-x/README2.txt, but "
        "polyoxide-x/README.txt holds no item to compare",
        "`[literals]` replaces 'never', but no baseline literal it applies to holds it",
        "`[paths]` maps `polyoxide_z::Gone`, but no baseline path or identifier it covers "
        "was rewritten"]


@pytest.mark.parametrize(("attribute", "kept"), [
    ('#[cfg(feature = "m")]', None),
    ('#![cfg(feature = "m")]', None),
    ('#[cfg(all(feature = "m", test))]', "# [ cfg ( test ) ]"),
    ('#[cfg(all(feature = "m", feature = "n", unix))]',
     '# [ cfg ( all ( feature = "n" , unix ) ) ]'),
    ('#[cfg(all(feature = "m"))]', None),
    ('#[cfg(any(feature = "m", test))]', '#[cfg(any(feature = "m", test))]'),
    ('#[cfg(feature = "n")]', '#[cfg(feature = "n")]'),
    ('#[cfg(not(feature = "m"))]', '#[cfg(not(feature = "m"))]'),
    ('#[doc(hidden)]', '#[doc(hidden)]'),
])
def test_a_gate_drops_exactly_its_features_cfg(attribute: str, kept: str | None) -> None:
    assert body_diff.drop_feature_gate(attribute, "m") == kept


ANY_MN = 'any(feature = "m", feature = "n")'


@pytest.mark.parametrize(("attribute", "kept"), [
    ('#[cfg(any(feature = "m", feature = "n"))]', None),
    ('#![cfg(any(feature="m",feature="n"))]', None),
    ('#[cfg(all(test, any(feature = "m", feature = "n")))]', "# [ cfg ( test ) ]"),
    ('#[cfg(all(any(feature = "m", feature = "n")))]', None),
    # Another order, another operand or the feature alone is another predicate.
    ('#[cfg(any(feature = "n", feature = "m"))]', '#[cfg(any(feature = "n", feature = "m"))]'),
    ('#[cfg(any(feature = "m", feature = "n", test))]',
     '#[cfg(any(feature = "m", feature = "n", test))]'),
    ('#[cfg(feature = "m")]', '#[cfg(feature = "m")]'),
    ('#[cfg(not(any(feature = "m", feature = "n")))]',
     '#[cfg(not(any(feature = "m", feature = "n")))]'),
    ('#[cfg_attr(any(feature = "m", feature = "n"), derive(Debug))]',
     '#[cfg_attr(any(feature = "m", feature = "n"), derive(Debug))]'),
])
def test_a_predicate_gate_drops_exactly_that_cfg(attribute: str, kept: str | None) -> None:
    assert body_diff.drop_feature_gate(attribute, ANY_MN) == kept


def test_a_feature_and_a_predicate_are_told_apart() -> None:
    assert not body_diff.is_predicate("clob-ws")
    assert body_diff.is_predicate(ANY_MN)
    assert body_diff.is_predicate('feature = "m"')
    assert body_diff.drop_feature_gate('#[cfg(feature = "m")]', 'feature = "m"') is None


ROOT_LIB = "pub fn f() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n"
LIVE = '[[test]]\nname = "live"\npath = "tests/live.rs"\n'


def test_a_crate_root_moved_under_a_module_feature_needs_a_gate_entry(repo: Path) -> None:
    baseline(repo, {"polyoxide-x/Cargo.toml": package("polyoxide-x") + LIVE,
                    "polyoxide-x/src/lib.rs": ROOT_LIB,
                    "polyoxide-x/tests/live.rs": "#[test]\nfn live() {}\n"})
    edit(repo, {"polyoxide-x/Cargo.toml": None, "polyoxide-x/src/lib.rs": None,
                "polyoxide-x/tests/live.rs": None,
                "polyoxide-y/Cargo.toml": package("polyoxide-y") + LIVE.replace(
                    "live", "live_m") + 'required-features = ["m"]\n',
                "polyoxide-y/src/lib.rs": "#[cfg(feature = \"m\")]\npub mod m;\n",
                "polyoxide-y/src/m/mod.rs": ROOT_LIB,
                "polyoxide-y/tests/live_m.rs": "#[test]\nfn live() {}\n"})
    rows = [Row("polyoxide_x", "polyoxide_y::m", None)]
    assert sorted(d.old.where for d in compare(repo, rows).diffs) == [
        "polyoxide-x/src/lib.rs:5 fn tests::t", "polyoxide-x/tests/live.rs target [[test]]"]
    gated = Config(gates={"polyoxide-y": ("m",)})
    assert compare(repo, rows, gated).status == 0
    stale = Config(gates={"polyoxide-y": ("m",), "polyoxide-y/src/z/": ("m", "q")})
    assert compare(repo, rows, stale).problems == [
        "`[gates]` drops the `m` gate under polyoxide-y/src/z/, but no item or stanza at "
        "HEAD there has it",
        "`[gates]` drops the `q` gate under polyoxide-y/src/z/, but no item or stanza at "
        "HEAD there has it"]


SHARED_LIB = ('#[cfg(any(feature = "m", feature = "n"))]\npub mod a;\n'
              '#[cfg(feature = "m")]\npub mod b;\n')


def test_a_module_under_a_predicate_needs_a_predicate_gate_entry(repo: Path) -> None:
    """A file moved under a `mod x;` gated by a predicate, which no feature entry can
    drop; the entry must fire, and drops nothing it does not name."""
    baseline(repo, {"polyoxide-x/Cargo.toml": package("polyoxide-x"),
                    "polyoxide-x/src/lib.rs": "pub mod a;\npub mod b;\n",
                    "polyoxide-x/src/a.rs": ROOT_LIB, "polyoxide-x/src/b.rs": ROOT_LIB})
    edit(repo, {"polyoxide-x/Cargo.toml": None, "polyoxide-x/src/lib.rs": None,
                "polyoxide-x/src/a.rs": None, "polyoxide-x/src/b.rs": None,
                "polyoxide-y/Cargo.toml": package("polyoxide-y"),
                "polyoxide-y/src/lib.rs": SHARED_LIB,
                "polyoxide-y/src/a.rs": ROOT_LIB, "polyoxide-y/src/b.rs": ROOT_LIB})
    rows = [Row("polyoxide_x", "polyoxide_y", None)]
    assert sorted(d.old.where for d in compare(repo, rows).diffs) == [
        "polyoxide-x/src/a.rs:5 fn tests::t", "polyoxide-x/src/b.rs:5 fn tests::t"]
    # The predicate drops `a`'s gate and not `b`'s, which only a feature entry drops.
    only = Config(gates={"polyoxide-y/src/": (ANY_MN,)})
    assert [d.old.where for d in compare(repo, rows, only).diffs] == [
        "polyoxide-x/src/b.rs:5 fn tests::t"]
    both = Config(gates={"polyoxide-y/src/a.rs": (ANY_MN,), "polyoxide-y/src/b.rs": ("m",)})
    assert compare(repo, rows, both).status == 0
    stale = Config(gates={"polyoxide-y/src/b.rs": ("m", ANY_MN), "polyoxide-y/src/a.rs": (ANY_MN,)})
    assert compare(repo, rows, stale).problems == [
        f"`[gates]` drops `cfg({ANY_MN})` under polyoxide-y/src/b.rs, but no item or stanza "
        f"at HEAD there has it"]


def test_a_baseline_the_checkout_lacks_cannot_be_decided(repo: Path) -> None:
    baseline(repo, TREE)
    with pytest.raises(BodyDiffError, match="v9 is not a commit in this checkout"):
        body_diff.compare(repo, "v9", (), Config(), DRIFT_ROWS)


def test_a_key_twice_at_head_cannot_be_decided(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, {"polyoxide-x/tests/mock_api.rs": X_TESTS + "fn client() {}\n"})
    with pytest.raises(BodyDiffError, match="HEAD: polyoxide-x/tests/mock_api.rs:18 has a "
                                            "second fn client"):
        compare(repo)


# --- the command line --------------------------------------------------------


def manifest(rows: list[str]) -> str:
    """The real manifest with `v0` as its baseline and `rows` under `## Paths`."""
    text = (REPO / "docs" / "rename-manifest.md").read_text()
    text = text.replace("baseline: v0.39.0", "baseline: v0", 1)
    return text.replace("## Paths\n", "## Paths\n\n" + "".join(f"- {r}\n" for r in rows), 1)


CLI_TREE = {**TREE, "ci/test-body-diff.toml": "[files]\n\n[literals]\n",
            "docs/rename-manifest.md": manifest(["`polyoxide_x` → `polyoxide_y::x` Story 5.4."])}


def test_main_reads_the_rows_through_the_manifest(repo: Path, capsys) -> None:
    baseline(repo, CLI_TREE)
    edit(repo, moved())
    assert body_diff.main([], root=repo) == 0
    assert "4 paired, 0 differ" in capsys.readouterr().out
    edit(repo, {"docs/rename-manifest.md": manifest([])})
    assert body_diff.main(["--baseline", "v0"], root=repo) == 1


def test_main_cannot_decide_without_its_inputs(repo: Path, capsys) -> None:
    baseline(repo, CLI_TREE)
    edit(repo, {"ci/test-body-diff.toml": "[files\n"})
    assert body_diff.main([], root=repo) == 2
    assert "::error::ci/test-body-diff.toml is not valid TOML" in capsys.readouterr().err
    edit(repo, {"ci/test-body-diff.toml": "", "docs/rename-manifest.md": None})
    assert body_diff.main([], root=repo) == 2
    assert "could not read docs/rename-manifest.md" in capsys.readouterr().err


# --- the real tree -----------------------------------------------------------


def test_the_checked_in_config_reads_and_cites_only_drift_rows() -> None:
    config = body_diff.read_config(REPO)
    rows = body_diff.drift_rows((REPO / body_diff.DIVERGENCES).read_text())
    assert rows == {f"R{n}" for n in range(1, 14)}
    assert all(entry.row in rows for entry in config.drift)
    # S2's DRIFT rows are R6, rtds' `test-fixtures` becoming `test-server`, and
    # R12, the live tests of the two data hosts Polymarket retired. S3 adds R13,
    # `Markets::get_many` sending `closed=any` where it fanned out two requests.
    assert {entry.row for entry in config.drift} <= {"R6", "R12", "R13"}
