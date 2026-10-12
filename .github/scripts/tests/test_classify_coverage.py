"""Every public error type is listed in its crate's `Classify` assertion.

Each crate's lib.rs holds

    const _: fn() = || {
        fn is<T: polyoxide_venue::Classify>() {}
        is::<ApiError>();
        ...
    };

so a listed type without the impl fails the build (`polyoxide-venue`, which defines
the trait, names it `crate::Classify`). A type nobody listed fails
nothing, and that is the half these tests close: they sweep every `pub enum` and
`pub struct` under `polyoxide*/src` that is an error, and fail naming each one
its crate's assertion leaves out.

A type is an error when its name ends in `Error`, when it derives `Error`
(`BurstCapacityExceeded` is found only this way), when its crate implements
`std::error::Error` for it by hand, or when it is one of the auxiliary names
below. `polyoxide-venue` is swept too, with one exemption: `ClassifiedError`
must not implement `Classify`, or the blanket `From` would overlap
`From<T> for T`. So is `polyoxide-ws`, whose `WsError` is exempt: venue does
not depend on ws (AD-2) and ws does not depend on venue, so neither can
implement the trait for it, and in a venue crate both are foreign (the orphan
rule). Each venue crate classifies it through its own enum that wraps it, with
`polyoxide_venue::impl_ws_classification!`.

That half has its own fence (Story 4.10): a type with a `WsError` field, of
any visibility, must sit in a file that expands the macro, since the fence
checks the file that expands it, and that file must call each of the macro's
`ws_class`, `ws_is_fault` and `ws_retry_after` in code, not in a comment.
Nothing else guards the calls: a function expanded from another crate's
`macro_rules!` and never called builds clean under `-D warnings` and clippy.

A doc comment or a `pub(crate)` item is not swept for the assertion. Gating is
not checked here: an assertion gated differently from its type fails
`cargo hack --each-feature`.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
# The vocabulary crate, which defines `Classify`.
VOCABULARY = "polyoxide-venue"
# The socket kit, which cannot see `Classify`.
SOCKET_KIT = "polyoxide-ws"
# (crate, type) pairs that do not implement `Classify`: `ClassifiedError` must
# not, and no crate can implement it for `WsError` (neither venue nor ws depends
# on the other, and in a venue crate both are foreign), so each venue's enum
# that wraps it classifies it.
EXEMPT = frozenset({(VOCABULARY, "ClassifiedError"), (SOCKET_KIT, "WsError")})
# Public error types whose names do not end in `Error`.
AUXILIARY = frozenset({"UnknownVariant", "InvalidSymbol", "InvalidStreamName", "BurstCapacityExceeded"})

ITEM = re.compile(r"^\s*pub (?:enum|struct) (\w+)")
# Any type, of any visibility, for the `WsError` fence.
ANY_ITEM = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:enum|struct) (\w+)")
WS_ERROR = re.compile(r"\bWsError\b")
# The macro expanded as an item, not named in a comment.
EXPANDS = re.compile(r"\bimpl_ws_classification!\s*\(\s*\)\s*;")
# The functions the expansion defines, each of which its file must call.
TABLE = ("ws_class", "ws_is_fault", "ws_retry_after")
COMMENT = re.compile(r"//.*$")
DERIVES_ERROR = re.compile(r"#\[derive\([^)]*\bError\b[^)]*\)\]", re.S)
IMPLS_ERROR = re.compile(r"\bimpl(?:<[^>]*>)?\s+(?:(?:std|core)::error::)?Error\s+for\s+(\w+)")
# The vocabulary crate names its own trait as `crate::Classify`.
ASSERTION = re.compile(r"fn is<T: (?:polyoxide_venue|crate)::Classify>\(\) \{\}(.*?)\n\};", re.S)
LISTED = re.compile(r"\bis::<([\w:]+)>\(\)")


def _attributes(lines: list[str], index: int) -> str:
    """The attributes and doc comment above line `index`, back to a blank line
    or the end of the previous item."""
    above = []
    for line in reversed(lines[:index]):
        stripped = line.strip()
        if not stripped:
            break
        if not stripped.startswith(("#", "//")) and stripped.endswith(("}", ";")):
            break
        above.append(line)
    return "\n".join(reversed(above))


def _read(path: Path) -> str:
    # The sources hold non-ASCII text (`USDⓈ-M`), which a runner whose locale
    # is not UTF-8 would fail to decode.
    return path.read_text(encoding="utf-8")


def error_types(src: Path) -> dict[str, Path]:
    """Each public error type under `src`, with the file that declares it."""
    files = sorted(src.rglob("*.rs"))
    implemented = {name for path in files for name in IMPLS_ERROR.findall(_read(path))}
    found = {}
    for path in files:
        lines = _read(path).splitlines()
        for index, line in enumerate(lines):
            item = ITEM.match(line)
            if not item:
                continue
            name = item[1]
            if (name.endswith("Error") or name in AUXILIARY or name in implemented
                    or DERIVES_ERROR.search(_attributes(lines, index))):
                found[name] = path
    return found


def asserted(lib: Path) -> set[str] | None:
    """The type names lib.rs's assertion lists, or `None` when it has none."""
    match = ASSERTION.search(_read(lib)) if lib.exists() else None
    if match is None:
        return None
    return {path.rsplit("::", 1)[-1] for path in LISTED.findall(match[1])}


def unlisted(root: Path) -> list[str]:
    """`<crate>: <type> (<file>)` for each error type its crate does not list."""
    problems = []
    for crate in sorted(root.glob("polyoxide*/")):
        src = crate / "src"
        if not src.is_dir():
            continue
        types = error_types(src)
        listed = asserted(src / "lib.rs") or set()
        for name, path in sorted(types.items()):
            if name not in listed and (crate.name, name) not in EXEMPT:
                problems.append(f"{crate.name}: {name} ({path.relative_to(root).as_posix()})")
    return problems


def _code(line: str) -> str:
    """`line` without its comment, doc comments included."""
    return COMMENT.sub("", line)


def _body(lines: list[str], index: int) -> str:
    """The code of the item declared on line `index`, comments left out: up to
    its `;` for a unit or tuple struct, or to the brace that closes it."""
    depth, body = 0, []
    for line in lines[index:]:
        code = _code(line)
        body.append(code)
        depth += code.count("{") - code.count("}")
        if depth <= 0 and ("}" in code or code.rstrip().endswith(";")):
            break
    return "\n".join(body)


def ws_wrappers(src: Path) -> list[tuple[str, Path]]:
    """Each type under `src` with a `WsError` field, of any visibility, with the
    file that declares it: every one, two of the same name in different files
    included."""
    found = []
    for path in sorted(src.rglob("*.rs")):
        lines = _read(path).splitlines()
        for index, line in enumerate(lines):
            item = ANY_ITEM.match(_code(line))
            if item and WS_ERROR.search(_body(lines, index)):
                found.append((item[1], path))
    return found


def _source(path: Path) -> str:
    """The file's code, comments left out."""
    return "\n".join(_code(line) for line in _read(path).splitlines())


def expands_the_table(path: Path) -> bool:
    """Whether the file expands `impl_ws_classification!` as code."""
    return EXPANDS.search(_source(path)) is not None


def uncalled(path: Path) -> list[str]:
    """The table's functions the file never calls in code. A definition of its
    own, `fn ws_class(`, is not a call."""
    code = _source(path)
    return [name for name in TABLE
            if not re.search(rf"(?<!fn )\b{name}\s*\(", code)]


def _swept(root: Path):
    """Each crate's name and `src`, the kit, which defines `WsError`, left out."""
    for crate in sorted(root.glob("polyoxide*/")):
        src = crate / "src"
        if crate.name != SOCKET_KIT and src.is_dir():
            yield crate.name, src


def unexpanded(root: Path) -> list[str]:
    """`<crate>: <type> (<file>)` for each `WsError` wrapper whose file does not
    expand the socket table."""
    problems = []
    for crate, src in _swept(root):
        for name, path in sorted(ws_wrappers(src)):
            if not expands_the_table(path):
                problems.append(f"{crate}: {name} ({path.relative_to(root).as_posix()})")
    return problems


def tables_not_called(root: Path) -> list[str]:
    """`<crate>: <file> does not call <function>` for each function of the socket
    table that a file expanding it never calls."""
    problems = []
    for crate, src in _swept(root):
        for path in sorted(src.rglob("*.rs")):
            if expands_the_table(path):
                problems.extend(f"{crate}: {path.relative_to(root).as_posix()} does not call {name}"
                                for name in uncalled(path))
    return problems


def test_every_public_error_type_is_listed_in_its_crates_assertion() -> None:
    problems = unlisted(REPO)
    assert not problems, (
        "these public error types are missing from their crate's `const _` Classify "
        "assertion in lib.rs; implement `polyoxide_venue::Classify` and list them:\n  "
        + "\n  ".join(problems)
    )


@pytest.mark.parametrize(("crate", "names"), [
    ("polyoxide-core", {"ApiError", "KeychainError"}),
    ("polyoxide-polymarket", {"BurstCapacityExceeded"}),
    ("polyoxide-data", {"DataApiError", "V2Error"}),
    ("polyoxide-clob", {"ClobError", "ParseTickSizeError", "WebSocketError"}),
    ("polyoxide-perps", {"PerpsError", "VenueError", "PerpsWsError"}),
    ("polyoxide-binance", {"BinanceError", "InvalidSymbol", "UsdmWsError", "InvalidStreamName"}),
    (VOCABULARY, {"UnknownVariant"}),
    ("polyoxide", {"PolymarketError"}),
])
def test_the_sweep_finds_the_known_types(crate: str, names: set[str]) -> None:
    # A sweep that matched nothing would pass the test above vacuously.
    assert names <= set(error_types(REPO / crate / "src"))


def test_the_vocabulary_crate_is_swept_but_classified_error_is_exempt() -> None:
    assert "ClassifiedError" in error_types(REPO / VOCABULARY / "src")
    assert not any(p.startswith(f"{VOCABULARY}:") for p in unlisted(REPO))


def test_the_socket_kit_is_swept_but_ws_error_is_exempt() -> None:
    assert "WsError" in error_types(REPO / SOCKET_KIT / "src")
    assert not any(p.startswith(f"{SOCKET_KIT}:") for p in unlisted(REPO))


def test_every_ws_error_wrapper_expands_the_socket_table() -> None:
    problems = unexpanded(REPO)
    assert not problems, (
        "these types wrap `polyoxide_ws::WsError` in a file that does not expand "
        "`polyoxide_venue::impl_ws_classification!();`; expand it there and classify the "
        "field through `ws_class`, `ws_is_fault` and `ws_retry_after`:\n  "
        + "\n  ".join(problems)
    )


def test_every_file_that_expands_the_socket_table_calls_all_of_it() -> None:
    problems = tables_not_called(REPO)
    assert not problems, (
        "these files expand `polyoxide_venue::impl_ws_classification!();` and never call "
        "one of its functions, which no lint reports; call it from the `Classify` impl:\n  "
        + "\n  ".join(problems)
    )


@pytest.mark.parametrize(("crate", "name"), [
    ("polyoxide-clob", "WebSocketError"),
    ("polyoxide-perps", "PerpsWsError"),
    ("polyoxide-binance", "UsdmWsError"),
    ("polyoxide-rtds", "RtdsError"),
    ("polyoxide-sports", "SportsError"),
])
def test_the_ws_sweep_finds_every_socket_error(crate: str, name: str) -> None:
    # A sweep that matched nothing would pass the fences above vacuously.
    [path] = [path for found, path in ws_wrappers(REPO / crate / "src") if found == name]
    assert expands_the_table(path)
    assert uncalled(path) == []


WS_WRAPPERS = """\
use polyoxide_ws::WsError;

/// A doc comment naming `WsError` is not a field.
pub enum Quiet {
    Stalled,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("{0}")]
    Connection(#[from] WsError),
}

pub(crate) struct Tuple(polyoxide_ws::WsError);

struct Named {
    source: WsError,
    // WsError in a comment
}
"""


def test_a_ws_error_field_without_the_table_is_caught(tmp_path: Path) -> None:
    src = tmp_path / "polyoxide-feed" / "src"
    src.mkdir(parents=True)
    (src / "error.rs").write_text(WS_WRAPPERS)
    # Named in a comment, which expands nothing.
    (src / "other.rs").write_text("// polyoxide_venue::impl_ws_classification!();\n"
                                  "pub struct Other { inner: WsError }\n")
    assert {name for name, _ in ws_wrappers(src)} == {"FeedError", "Tuple", "Named", "Other"}
    assert unexpanded(tmp_path) == [
        "polyoxide-feed: FeedError (polyoxide-feed/src/error.rs)",
        "polyoxide-feed: Named (polyoxide-feed/src/error.rs)",
        "polyoxide-feed: Other (polyoxide-feed/src/other.rs)",
        "polyoxide-feed: Tuple (polyoxide-feed/src/error.rs)",
    ]

    (src / "error.rs").write_text("polyoxide_venue::impl_ws_classification!();\n" + WS_WRAPPERS)
    assert unexpanded(tmp_path) == ["polyoxide-feed: Other (polyoxide-feed/src/other.rs)"]


def test_two_wrappers_of_one_name_are_each_checked(tmp_path: Path) -> None:
    src = tmp_path / "polyoxide-feed" / "src"
    (src / "a").mkdir(parents=True)
    (src / "b").mkdir()
    (src / "a" / "error.rs").write_text(
        "polyoxide_venue::impl_ws_classification!();\npub enum FeedError { C(WsError) }\n")
    (src / "b" / "error.rs").write_text("pub enum FeedError { C(WsError) }\n")
    assert len(ws_wrappers(src)) == 2
    assert unexpanded(tmp_path) == ["polyoxide-feed: FeedError (polyoxide-feed/src/b/error.rs)"]


CALLS_TWO = """\
polyoxide_venue::impl_ws_classification!();

pub enum FeedError {
    Connection(WsError),
}

impl Classify for FeedError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => ws_class(err),
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connection(err) if !ws_is_fault(err))
    }

    // ws_retry_after(err) in a comment is not a call.
}

/// So is `ws_retry_after(err)` in a doc comment, and a function of its own.
fn ws_retry_after() {}
"""


def test_a_file_that_forgets_a_table_function_is_caught(tmp_path: Path) -> None:
    src = tmp_path / "polyoxide-feed" / "src"
    src.mkdir(parents=True)
    (src / "error.rs").write_text(CALLS_TWO)
    assert unexpanded(tmp_path) == []
    assert tables_not_called(tmp_path) == [
        "polyoxide-feed: polyoxide-feed/src/error.rs does not call ws_retry_after",
    ]

    (src / "error.rs").write_text(CALLS_TWO.replace(
        "    // ws_retry_after(err) in a comment is not a call.\n",
        "    fn retry_after(&self) -> Option<Duration> {\n"
        "        match self { Self::Connection(err) => ws_retry_after(err) }\n"
        "    }\n"))
    assert tables_not_called(tmp_path) == []


def test_the_socket_kit_is_not_swept_for_the_table(tmp_path: Path) -> None:
    src = tmp_path / SOCKET_KIT / "src"
    src.mkdir(parents=True)
    (src / "error.rs").write_text("pub struct Wrapper(WsError);\n")
    assert unexpanded(tmp_path) == []


def test_the_socket_kit_exempts_only_ws_error(tmp_path: Path) -> None:
    _crate(tmp_path, SOCKET_KIT, None, "pub struct WsError;\npub enum OtherError {}\n")
    assert unlisted(tmp_path) == [f"{SOCKET_KIT}: OtherError ({SOCKET_KIT}/src/error.rs)"]


LIB = """\
pub mod error;

const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<FakeError>();
    #[cfg(feature = "ws")]
    is::<ws::FakeWsError>();
};
"""

ERRORS = """\
/// A doc comment naming `pub enum DocError` is not an item.
#[derive(Debug, thiserror::Error)]
pub enum FakeError {}

#[derive(Debug, thiserror::Error)]
pub enum FakeWsError {}

pub(crate) struct PrivateError;

/// No `Error` in the name, but an error all the same.
#[derive(Debug, Clone, thiserror::Error)]
#[error(
    "costs {cost}; \\
     too much"
)]
pub struct Overdraft {
    pub cost: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Receipt;

pub struct UnknownVariant {
    pub value: String,
}

pub struct NewError;
"""


def _crate(root: Path, name: str, lib: str | None, errors: str) -> None:
    src = root / name / "src"
    src.mkdir(parents=True)
    if lib is not None:
        (src / "lib.rs").write_text(lib)
    (src / "error.rs").write_text(errors)


def test_a_new_error_type_is_caught(tmp_path: Path) -> None:
    _crate(tmp_path, "polyoxide-fake", LIB, ERRORS)
    assert unlisted(tmp_path) == [
        "polyoxide-fake: NewError (polyoxide-fake/src/error.rs)",
        "polyoxide-fake: Overdraft (polyoxide-fake/src/error.rs)",
        "polyoxide-fake: UnknownVariant (polyoxide-fake/src/error.rs)",
    ]


def test_a_crate_without_an_assertion_reports_every_type(tmp_path: Path) -> None:
    _crate(tmp_path, "polyoxide-bare", None, "pub enum BareError {}\n")
    assert unlisted(tmp_path) == ["polyoxide-bare: BareError (polyoxide-bare/src/error.rs)"]


def test_a_hand_written_error_impl_is_caught(tmp_path: Path) -> None:
    _crate(tmp_path, "polyoxide-manual", None, """\
use std::error::Error;

pub struct Overflow;
impl Error for Overflow {}

pub struct Underflow;
impl std::error::Error for Underflow {}

pub struct Wrapped<T>(T);
impl<T: std::fmt::Debug> core::error::Error for Wrapped<T> {}

struct Hidden;
impl Error for Hidden {}

pub struct Plain;
""")
    assert unlisted(tmp_path) == [
        "polyoxide-manual: Overflow (polyoxide-manual/src/error.rs)",
        "polyoxide-manual: Underflow (polyoxide-manual/src/error.rs)",
        "polyoxide-manual: Wrapped (polyoxide-manual/src/error.rs)",
    ]


def test_the_vocabulary_crate_s_own_assertion_names_crate_classify(tmp_path: Path) -> None:
    lib = LIB.replace("polyoxide_venue::Classify", "crate::Classify")
    _crate(tmp_path, VOCABULARY, lib, "pub struct ClassifiedError;\npub enum FakeError {}\n"
           "pub enum NewError {}\n")
    assert asserted(tmp_path / VOCABULARY / "src" / "lib.rs") == {"FakeError", "FakeWsError"}
    assert unlisted(tmp_path) == [f"{VOCABULARY}: NewError ({VOCABULARY}/src/error.rs)"]


def test_the_vocabulary_crate_exempts_only_classified_error(tmp_path: Path) -> None:
    _crate(tmp_path, VOCABULARY, None, "pub struct ClassifiedError;\npub enum OtherError {}\n")
    assert unlisted(tmp_path) == [f"{VOCABULARY}: OtherError ({VOCABULARY}/src/error.rs)"]


def test_sources_are_read_as_utf8(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    _crate(tmp_path, "polyoxide-usdm", LIB, "/// USDⓈ-M\npub enum FakeError {}\n")
    # As a runner with an ASCII locale would read them by default.
    real = Path.read_text
    monkeypatch.setattr(Path, "read_text", lambda self, encoding=None, errors=None:
                        real(self, encoding=encoding or "ascii", errors=errors))
    assert unlisted(tmp_path) == []
