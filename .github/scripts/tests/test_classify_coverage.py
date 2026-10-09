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
`From<T> for T`.

A doc comment or a `pub(crate)` item is not swept. Gating is not checked here:
an assertion gated differently from its type fails `cargo hack --each-feature`.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
# The vocabulary crate, which defines `Classify`.
VOCABULARY = "polyoxide-venue"
# (crate, type) pairs that must not implement `Classify`.
EXEMPT = frozenset({(VOCABULARY, "ClassifiedError")})
# Public error types whose names do not end in `Error`.
AUXILIARY = frozenset({"UnknownVariant", "InvalidSymbol", "InvalidStreamName", "BurstCapacityExceeded"})

ITEM = re.compile(r"^\s*pub (?:enum|struct) (\w+)")
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


def test_every_public_error_type_is_listed_in_its_crates_assertion() -> None:
    problems = unlisted(REPO)
    assert not problems, (
        "these public error types are missing from their crate's `const _` Classify "
        "assertion in lib.rs; implement `polyoxide_venue::Classify` and list them:\n  "
        + "\n  ".join(problems)
    )


@pytest.mark.parametrize(("crate", "names"), [
    ("polyoxide-core", {"ApiError", "BurstCapacityExceeded", "KeychainError"}),
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
