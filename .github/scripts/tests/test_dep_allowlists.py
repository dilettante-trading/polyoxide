"""Every crate a socket-only member builds, pinned per member and feature set.

`ci/dep-allowlists/<crate>.txt` lists the crate names in

    cargo tree -p <crate> -e normal --target x86_64-unknown-linux-gnu

sorted and unique, the crate itself left out, and `<crate>@<f1,f2>.txt` the
same with `--features f1,f2`. A crate that joins the tree fails here, named,
and so does one that leaves it, so the file is always the tree (AD-2, NFR2): a
credential-free feed that builds an HTTP or signing stack is a red build, not a
heavier release.

`-e normal` leaves dev-dependencies out of both the listing and the feature
resolution, so the tree is the normal dependencies a consumer builds into its
program. It also leaves out build-dependencies, which run only at build time:
ring's `cc`, for one, is built but is not on the list. The target is fixed so a
developer on another platform reads the same tree CI does.

No allowlist may admit a crate on the socket-only deny-list, so a regenerated
file cannot quietly accept one, and `polyoxide-ws` may build neither
`polyoxide-core` nor `polyoxide-venue`.

Regenerate every file after a deliberate change, from `.github/scripts`:

    uv run python -m tests.test_dep_allowlists --write

and say in the commit why the tree changed.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
ALLOWLISTS = REPO / "ci" / "dep-allowlists"
TARGET = "x86_64-unknown-linux-gnu"
# The socket-only deny-list: an HTTP client, a signing stack, core and its rate
# limiter, and the keychain.
DENIED = frozenset({"reqwest", "polyoxide-core", "governor", "hmac", "sha2", "keyring"})
DENIED_PREFIXES = ("alloy",)
# Fences of one crate's own.
DENIED_FOR = {"polyoxide-ws": frozenset({"polyoxide-venue"})}

HEADER = """\
# The crates `cargo tree -p {crate} -e normal --target {target}{features}` builds,
# sorted and unique, the crate itself left out. CI's scripts job fails on any
# difference (.github/scripts/tests/test_dep_allowlists.py). Regenerate with
# `uv run python -m tests.test_dep_allowlists --write` in .github/scripts, and
# say in the commit why the tree changed.
"""


def parse_name(stem: str) -> tuple[str, tuple[str, ...]]:
    """The crate and the features a file's name selects."""
    crate, _, features = stem.partition("@")
    return crate, tuple(f for f in features.split(",") if f)


def parse_tree(output: str, root: str) -> set[str]:
    """The crate names in `cargo tree --prefix none` output, the root left out.

    Each line is `<name> v<version> [(<path>)] [(proc-macro)] [(*)]`, and a
    crate built twice at two versions is one name.
    """
    names = {line.split()[0] for line in output.splitlines() if line.strip()}
    names.discard(root)
    return names


def read_allowlist(path: Path) -> list[str]:
    """The names a file lists, in file order, comments and blank lines skipped."""
    lines = (line.strip() for line in path.read_text(encoding="utf-8").splitlines())
    return [line for line in lines if line and not line.startswith("#")]


def cargo_tree(crate: str, features: tuple[str, ...]) -> set[str]:
    """The crate names `crate` builds with `features`."""
    command = ["cargo", "tree", "-p", crate, "-e", "normal", "--prefix", "none",
               "--target", TARGET, "--locked"]
    if features:
        command += ["--features", ",".join(features)]
    result = subprocess.run(command, cwd=REPO, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise AssertionError(f"`{' '.join(command)}` failed:\n{result.stderr}")
    return parse_tree(result.stdout, crate)


def problems(label: str, listed: list[str], built: set[str]) -> list[str]:
    """Each way `listed` is not exactly `built`, sorted, unique and complete."""
    found = []
    if listed != sorted(set(listed)):
        found.append(f"{label} is not sorted and unique")
    for name in sorted(built - set(listed)):
        found.append(f"{label} builds {name}, which its allowlist does not list")
    for name in sorted(set(listed) - built):
        found.append(f"{label} lists {name}, which it no longer builds")
    return found


def denied(crate: str, names: list[str]) -> list[str]:
    """Each name the deny-lists refuse `crate`."""
    refused = DENIED | DENIED_FOR.get(crate, frozenset())
    return sorted(n for n in names if n in refused or n.startswith(DENIED_PREFIXES))


def allowlists() -> list[Path]:
    return sorted(ALLOWLISTS.glob("*.txt"))


def write() -> None:
    """Rewrite every allowlist from the tree cargo reports now."""
    for path in allowlists():
        crate, features = parse_name(path.stem)
        named = f" --features {','.join(features)}" if features else ""
        header = HEADER.format(crate=crate, target=TARGET, features=named)
        path.write_text(header + "".join(f"{n}\n" for n in sorted(cargo_tree(crate, features))),
                        encoding="utf-8")


def test_the_allowlists_exist() -> None:
    # A glob that matched nothing would pass every test below vacuously.
    names = [p.name for p in allowlists()]
    assert {"polyoxide-ws.txt", "polyoxide-ws@test-server.txt"} <= set(names), names
    assert "tokio-tungstenite" in read_allowlist(ALLOWLISTS / "polyoxide-ws.txt")


@pytest.mark.parametrize("path", allowlists(), ids=lambda p: p.stem)
def test_each_allowlist_is_the_tree_cargo_builds(path: Path) -> None:
    crate, features = parse_name(path.stem)
    found = problems(path.name, read_allowlist(path), cargo_tree(crate, features))
    assert not found, "\n  ".join(["the tree and its allowlist differ:", *found])


@pytest.mark.parametrize("path", allowlists(), ids=lambda p: p.stem)
def test_no_allowlist_admits_a_denied_crate(path: Path) -> None:
    crate, _ = parse_name(path.stem)
    assert denied(crate, read_allowlist(path)) == []


TREE = """\
polyoxide-ws v0.38.2 (/repo/polyoxide-ws)
futures-util v0.3.32
futures-macro v0.3.32 (proc-macro)
proc-macro2 v1.0.95
tokio v1.52.0
tokio-tungstenite v0.26.2
tokio v1.52.0 (*)
rand v0.8.5
rand v0.9.4
"""


def test_the_parser_reads_cargo_tree_output() -> None:
    assert parse_tree(TREE, "polyoxide-ws") == {
        "futures-util", "futures-macro", "proc-macro2", "tokio", "tokio-tungstenite", "rand",
    }
    assert parse_tree("", "polyoxide-ws") == set()


def test_the_file_name_names_the_crate_and_its_features() -> None:
    assert parse_name("polyoxide-ws") == ("polyoxide-ws", ())
    assert parse_name("polyoxide-ws@test-server") == ("polyoxide-ws", ("test-server",))
    assert parse_name("polyoxide-rtds@a,b") == ("polyoxide-rtds", ("a", "b"))


def test_a_crate_that_joins_or_leaves_the_tree_is_named() -> None:
    listed = ["futures-util", "tokio", "tokio-tungstenite"]
    assert problems("polyoxide-ws.txt", listed, {"futures-util", "tokio", "tokio-tungstenite"}) == []
    assert problems("polyoxide-ws.txt", listed, {"futures-util", "reqwest", "tokio"}) == [
        "polyoxide-ws.txt builds reqwest, which its allowlist does not list",
        "polyoxide-ws.txt lists tokio-tungstenite, which it no longer builds",
    ]
    assert problems("x.txt", ["tokio", "futures-util"], {"tokio", "futures-util"}) == [
        "x.txt is not sorted and unique"
    ]


def test_the_deny_lists_refuse_their_crates(tmp_path: Path) -> None:
    names = ["alloy-primitives", "hmac", "polyoxide-venue", "rand", "reqwest", "tokio"]
    assert denied("polyoxide-ws", names) == ["alloy-primitives", "hmac", "polyoxide-venue", "reqwest"]
    # Only `polyoxide-ws` is fenced from venue; a socket crate builds on it.
    assert denied("polyoxide-rtds", names) == ["alloy-primitives", "hmac", "reqwest"]

    path = tmp_path / "polyoxide-ws.txt"
    path.write_text(HEADER + "\ntokio\n\n# a note\nrand\n", encoding="utf-8")
    assert read_allowlist(path) == ["tokio", "rand"]


if __name__ == "__main__":
    if sys.argv[1:] != ["--write"]:
        sys.exit("usage: python -m tests.test_dep_allowlists --write")
    write()
