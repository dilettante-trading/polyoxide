"""Every crate a fenced member builds, pinned per member and feature set.

`ci/dep-allowlists/<crate>.txt` lists the crate names in

    cargo tree -p <crate> -e normal --target x86_64-unknown-linux-gnu

sorted and unique, the crate itself left out: the tree a consumer builds by
default. `<crate>@<f1,f2>.txt` is the same with `--features f1,f2`, and a venue
crate held per feature (below) is built with `--no-default-features` too, so its
file names every feature it builds and the crate's defaults never widen it. A
crate that joins the tree fails here, named,
and so does one that leaves it, so the file is always the tree (AD-2, NFR2): a
credential-free feed that builds an HTTP or signing stack is a red build, not a
heavier release.

`-e normal` leaves dev-dependencies out of both the listing and the feature
resolution, so the tree is the normal dependencies a consumer builds into its
program. It also leaves out build-dependencies, which run only at build time:
ring's `cc`, for one, is built but is not on the list. The target is fixed so a
developer on another platform reads the same tree CI does.

No allowlist may admit a crate its deny-list refuses, so a regenerated file
cannot quietly accept one. The socket kit's files are held to the socket-only
deny-list, and `polyoxide-ws` may build neither `polyoxide-core` nor
`polyoxide-venue`. A venue crate's files are held per feature, after the
features each file names turn on the ones they imply: `polyoxide-polymarket`
builds `reqwest`, `polyoxide-core`, `governor` and `sha2`, the HTTP stack, only
with an HTTP module (`clob`, `gamma`, `data`, `relay` or `perps`), so its socket
modules `rtds` and `sports` build the socket-only tree; `alloy*` and `hmac`, the
signing stack, only with `clob` or `relay`; and `keyring` only with `keychain`.
`polyoxide-binance` builds `polyoxide-ws`, `tokio-tungstenite` and
`tungstenite`, the socket stack, only with `usdm-ws`, so its `@usdm` file is the
REST client alone. `rustls` is not on that rule: core's HTTP client builds it
for TLS. The `polyoxide` umbrella is held as Polymarket is, on its own feature
names: the HTTP stack only with an HTTP module of either venue
(`polymarket-clob`, `-gamma`, `-data`, `-relay` or `-perps`, or
`binance-usdm`), so its `@polymarket-rtds` and `@polymarket-sports` files are
the socket-only tree; the signing stack only with `polymarket-clob` or
`polymarket-relay`; and `keyring` only with `keychain`. Polymarket's `@perps`
file must hold `reqwest`, its `@clob` and `@relay` files `alloy`, and its
`@clob,keychain` file `keyring`, Binance's `@usdm-ws` file `polyoxide-ws`, and
the umbrella's `@polymarket-perps` file `reqwest`, its `@polymarket-clob` file
`alloy` and its `@polymarket-clob,keychain` file `keyring`, so no rule can pass
because nothing it admits is ever built.

`polyoxide-kalshi` is held per feature as well (D23): `reqwest`, `polyoxide-core`,
`governor` and `sha2` only with `predictions`, which every file names;
`polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `predictions-ws`;
and `alloy*`, `hmac` and `keyring` never, since Kalshi signs with `ring`. Its
`@predictions` file must exist, and its `@predictions-ws` file, once written, must
hold `polyoxide-ws`.

Regenerate every file after a deliberate change, from `.github/scripts`:

    uv run python -m tests.test_dep_allowlists --write

and say in the commit why the tree changed.
"""

from __future__ import annotations

import json
import subprocess
import sys
from functools import cache
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
SOCKET_ONLY = "the socket-only deny-list"


class Rule:
    """Crates a venue crate's file may build only when its features include one
    of `admitted_by`: those named in `names`, and those starting with a prefix."""

    def __init__(self, names: frozenset[str], prefixes: tuple[str, ...],
                 admitted_by: frozenset[str], says: str) -> None:
        self.names, self.prefixes, self.admitted_by, self.says = (
            names, prefixes, admitted_by, says)

    def refuses(self, name: str) -> bool:
        return name in self.names or name.startswith(self.prefixes)


# A venue crate's per-feature fence (NFR2), in place of the socket-only one: a
# socket module builds no HTTP stack, and no credential-free module builds the
# signing stack or the keychain.
PER_FEATURE = {
    "polyoxide-polymarket": (
        Rule(frozenset({"reqwest", "polyoxide-core", "governor", "sha2"}), (),
             frozenset({"clob", "gamma", "data", "relay", "perps"}),
             "reqwest, polyoxide-core, governor and sha2 only with an HTTP module"),
        Rule(frozenset({"hmac"}), ("alloy",), frozenset({"clob", "relay"}),
             "alloy* and hmac only with clob or relay"),
        Rule(frozenset({"keyring"}), (), frozenset({"keychain"}),
             "keyring only with keychain"),
    ),
    "polyoxide-binance": (
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"usdm-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws"),
    ),
    # Kalshi's REST module builds the HTTP stack and signs with ring, so no file of it admits
    # the Polymarket signing stack or the keychain; the socket stack is its order-book
    # stream's alone.
    "polyoxide-kalshi": (
        Rule(frozenset({"reqwest", "polyoxide-core", "governor", "sha2"}), (),
             frozenset({"predictions"}),
             "reqwest, polyoxide-core, governor and sha2 only with predictions"),
        Rule(frozenset({"hmac", "keyring"}), ("alloy",), frozenset(),
             "alloy*, hmac and keyring are never built"),
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"predictions-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with predictions-ws"),
    ),
    # The umbrella's socket-only deny-list, each part admitted by the features
    # that pass Polymarket's admitting modules through, and by Binance's HTTP
    # module: its socket passthroughs stay credential-free feeds (G2).
    "polyoxide": (
        Rule(frozenset({"reqwest", "polyoxide-core", "governor", "sha2"}), (),
             frozenset({"polymarket-clob", "polymarket-gamma", "polymarket-data",
                        "polymarket-relay", "polymarket-perps", "binance-usdm"}),
             "reqwest, polyoxide-core, governor and sha2 only with an HTTP module"),
        Rule(frozenset({"hmac"}), ("alloy",), frozenset({"polymarket-clob", "polymarket-relay"}),
             "alloy* and hmac only with polymarket-clob or polymarket-relay"),
        Rule(frozenset({"keyring"}), (), frozenset({"keychain"}),
             "keyring only with keychain"),
    ),
}
# The files that must build a crate a rule above admits, with that crate.
MUST_BUILD = {
    "polyoxide-polymarket@perps.txt": "reqwest",
    "polyoxide-polymarket@clob.txt": "alloy",
    "polyoxide-polymarket@relay.txt": "alloy",
    "polyoxide-polymarket@clob,keychain.txt": "keyring",
    "polyoxide-binance@usdm-ws.txt": "polyoxide-ws",
    "polyoxide@polymarket-perps.txt": "reqwest",
    "polyoxide@polymarket-clob.txt": "alloy",
    "polyoxide@polymarket-clob,keychain.txt": "keyring",
    "polyoxide-kalshi@predictions.txt": "reqwest",
}

HEADER = """\
# The crates `cargo tree -p {crate} -e normal{defaults} --target {target}{features}` builds,
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


def no_defaults(crate: str) -> list[str]:
    """`--no-default-features` for a crate held per feature, whose files name every
    feature they build; nothing for any other, whose bare file is its default build."""
    return ["--no-default-features"] if crate in PER_FEATURE else []


def cargo_tree(crate: str, features: tuple[str, ...]) -> set[str]:
    """The crate names `crate` builds with `features`."""
    command = ["cargo", "tree", "-p", crate, "-e", "normal", "--prefix", "none",
               *no_defaults(crate), "--target", TARGET, "--locked"]
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


@cache
def feature_table(crate: str) -> dict[str, tuple[str, ...]]:
    """`crate`'s `[features]`, from cargo metadata."""
    command = ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"]
    result = subprocess.run(command, cwd=REPO, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise AssertionError(f"`{' '.join(command)}` failed:\n{result.stderr}")
    package = next(p for p in json.loads(result.stdout)["packages"] if p["name"] == crate)
    return {name: tuple(items) for name, items in package["features"].items()}


def enabled(features: tuple[str, ...], table: dict[str, tuple[str, ...]]) -> set[str]:
    """`features` and every feature of the same crate they turn on, `default` not
    among them: an item naming a dependency (`dep:x`, `x/f`, `x?/f`) turns on none."""
    on, todo = set(), list(features)
    while todo:
        feature = todo.pop()
        if feature not in on:
            on.add(feature)
            todo += [f for f in table.get(feature, ()) if ":" not in f and "/" not in f]
    return on


def denied(crate: str, names: list[str], features: tuple[str, ...] = (),
           table: dict[str, tuple[str, ...]] | None = None) -> list[str]:
    """Each name `crate` built with `features` may not build, with the rule
    that refuses it. `table` is the crate's `[features]`, read from cargo
    metadata when a per-feature rule needs it and none is given."""
    rules = PER_FEATURE.get(crate)
    if rules is None:
        refused = DENIED | DENIED_FOR.get(crate, frozenset())
        return sorted(f"{n} ({SOCKET_ONLY})" for n in names
                      if n in refused or n.startswith(DENIED_PREFIXES))
    on = enabled(features, feature_table(crate) if table is None else table)
    return sorted(f"{n} ({rule.says})" for rule in rules if not on & rule.admitted_by
                  for n in names if rule.refuses(n))


def allowlists() -> list[Path]:
    return sorted(ALLOWLISTS.glob("*.txt"))


def write() -> None:
    """Rewrite every allowlist from the tree cargo reports now."""
    for path in allowlists():
        crate, features = parse_name(path.stem)
        named = f" --features {','.join(features)}" if features else ""
        defaults = "".join(f" {flag}" for flag in no_defaults(crate))
        header = HEADER.format(crate=crate, defaults=defaults, target=TARGET, features=named)
        path.write_text(header + "".join(f"{n}\n" for n in sorted(cargo_tree(crate, features))),
                        encoding="utf-8")


def test_the_allowlists_exist() -> None:
    # A glob that matched nothing would pass every test below vacuously, and so
    # would a venue crate whose credential-free modules have no file.
    names = [p.name for p in allowlists()]
    assert {"polyoxide-ws.txt", "polyoxide-ws@test-server.txt"} <= set(names), names
    assert "tokio-tungstenite" in read_allowlist(ALLOWLISTS / "polyoxide-ws.txt")
    modules = ("gamma", "data", "perps", "perps-ws", "clob", "clob-ws", "relay",
               "clob,keychain", "rtds", "rtds,test-server", "sports", "sports,test-server")
    assert {f"polyoxide-polymarket@{m}.txt" for m in modules} <= set(names), names
    assert {f"polyoxide-binance@{m}.txt" for m in ("usdm", "usdm-ws")} <= set(names), names
    assert "polyoxide-kalshi@predictions.txt" in names, names
    umbrella = ("polymarket-rtds", "polymarket-sports", "polymarket-perps", "polymarket-clob",
                "polymarket-clob,keychain")
    assert {f"polyoxide@{m}.txt" for m in umbrella} <= set(names), names


@pytest.mark.parametrize("name", sorted(MUST_BUILD))
def test_each_per_feature_rule_admits_what_it_guards(name: str) -> None:
    # The rules admit the HTTP stack with an HTTP module, alloy with clob or
    # relay, keyring with keychain and Binance's socket stack with usdm-ws; if
    # no file held them, a rule that never refuses anything could not be told
    # from one that holds.
    assert MUST_BUILD[name] in read_allowlist(ALLOWLISTS / name), name


@pytest.mark.parametrize("path", allowlists(), ids=lambda p: p.stem)
def test_each_allowlist_is_the_tree_cargo_builds(path: Path) -> None:
    crate, features = parse_name(path.stem)
    found = problems(path.name, read_allowlist(path), cargo_tree(crate, features))
    assert not found, "\n  ".join(["the tree and its allowlist differ:", *found])


@pytest.mark.parametrize("path", allowlists(), ids=lambda p: p.stem)
def test_no_allowlist_admits_a_denied_crate(path: Path) -> None:
    crate, features = parse_name(path.stem)
    refused = denied(crate, read_allowlist(path), features)
    assert refused == [], f"{path.name} builds what its deny-list refuses: {refused}"


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
    assert parse_name("polyoxide-polymarket@rtds,test-server") == (
        "polyoxide-polymarket", ("rtds", "test-server"))


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
    assert denied("polyoxide-ws", names) == [
        f"alloy-primitives ({SOCKET_ONLY})", f"hmac ({SOCKET_ONLY})",
        f"polyoxide-venue ({SOCKET_ONLY})", f"reqwest ({SOCKET_ONLY})"]
    # Only `polyoxide-ws` is fenced from venue; any other socket crate builds on it.
    assert denied("polyoxide-socket", names) == [
        f"alloy-primitives ({SOCKET_ONLY})", f"hmac ({SOCKET_ONLY})", f"reqwest ({SOCKET_ONLY})"]

    path = tmp_path / "polyoxide-ws.txt"
    path.write_text(HEADER + "\ntokio\n\n# a note\nrand\n", encoding="utf-8")
    assert read_allowlist(path) == ["tokio", "rand"]


# A venue crate's `[features]`, as cargo metadata gives it.
TABLE = {
    "default": ("clob", "gamma", "data"),
    "clob": ("dep:polyoxide-core", "dep:alloy", "dep:hmac", "tokio/time"),
    "clob-ws": ("clob", "dep:polyoxide-ws"),
    "gamma": ("dep:polyoxide-core",),
    "data": ("dep:polyoxide-core",),
    "relay": ("dep:alloy", "alloy?/provider-http", "dep:hmac"),
    "perps": ("dep:polyoxide-core", "polyoxide-venue/decimal"),
    "perps-ws": ("perps", "dep:polyoxide-ws", "tokio/time"),
    "rtds": ("dep:polyoxide-ws", "dep:tokio", "tokio/time"),
    "sports": ("dep:polyoxide-ws", "dep:tokio", "tokio/time"),
    "test-server": (),
    "keychain": ("polyoxide-core?/keychain",),
}
VENUE = "polyoxide-polymarket"
HTTP = "reqwest, polyoxide-core, governor and sha2 only with an HTTP module"
SIGNING = "alloy* and hmac only with clob or relay"
KEYCHAIN = "keyring only with keychain"


def test_a_credential_free_module_may_not_build_the_signing_stack() -> None:
    names = ["alloy", "alloy-primitives", "hmac", "keyring", "reqwest", "tokio"]
    assert denied(VENUE, names, ("data",), TABLE) == [
        f"alloy ({SIGNING})", f"alloy-primitives ({SIGNING})", f"hmac ({SIGNING})",
        f"keyring ({KEYCHAIN})"]
    # The HTTP stack is an HTTP module's own, credential-free or not.
    assert denied(VENUE, ["reqwest", "polyoxide-core", "governor", "sha2"], ("gamma",),
                  TABLE) == []
    # A crate's defaults admit nothing: a file names every feature it builds.
    assert denied(VENUE, ["alloy", "reqwest"], (), TABLE) == [
        f"alloy ({SIGNING})", f"reqwest ({HTTP})"]


HTTP_STACK = ["governor", "polyoxide-core", "reqwest", "sha2"]


def test_a_socket_module_may_not_build_the_http_stack() -> None:
    names = [*HTTP_STACK, "alloy-primitives", "hmac", "keyring", "polyoxide-ws", "tokio"]
    socket_only = [f"alloy-primitives ({SIGNING})", f"governor ({HTTP})", f"hmac ({SIGNING})",
                   f"keyring ({KEYCHAIN})", f"polyoxide-core ({HTTP})", f"reqwest ({HTTP})",
                   f"sha2 ({HTTP})"]
    for features in (("rtds",), ("sports",), ("rtds", "test-server"),
                     ("sports", "test-server"), ("rtds", "sports")):
        assert denied(VENUE, names, features, TABLE) == socket_only, features
    # `keychain` turns core's keychain on only where core is built, so it admits
    # no HTTP stack.
    assert denied(VENUE, HTTP_STACK, ("rtds", "keychain"), TABLE) == [
        f"{n} ({HTTP})" for n in HTTP_STACK]


def test_each_http_module_admits_the_http_stack() -> None:
    for features in (("clob",), ("gamma",), ("data",), ("relay",), ("perps",), ("perps-ws",),
                     ("clob-ws",), ("rtds", "perps"), ("sports", "gamma")):
        assert denied(VENUE, HTTP_STACK, features, TABLE) == [], features


def test_a_signing_module_admits_the_signing_stack_and_not_the_keychain() -> None:
    names = ["alloy", "alloy-primitives", "hmac", "keyring"]
    for features in (("clob",), ("relay",), ("gamma", "relay")):
        assert denied(VENUE, names, features, TABLE) == [f"keyring ({KEYCHAIN})"], features
    # `clob-ws` turns `clob` on, so it admits what `clob` does.
    assert denied(VENUE, names, ("clob-ws",), TABLE) == [f"keyring ({KEYCHAIN})"]
    assert denied(VENUE, names, ("clob", "keychain"), TABLE) == []
    # `keychain` admits the keychain and nothing else.
    assert denied(VENUE, names, ("data", "keychain"), TABLE) == [
        f"alloy ({SIGNING})", f"alloy-primitives ({SIGNING})", f"hmac ({SIGNING})"]


# A trimmed stand-in for Binance's `[features]`, in cargo metadata's form: the
# feature walk reads only the features each one turns on. What the real table's
# features turn on is checked against it below.
BINANCE_TABLE = {
    "default": ("usdm",),
    "usdm": ("dep:polyoxide-core", "polyoxide-venue/decimal", "dep:tokio"),
    "usdm-ws": ("usdm", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt"),
    "test-server": (),
}
BINANCE = "polyoxide-binance"
SOCKETS = "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws"
SOCKET_STACK = ["polyoxide-ws", "tokio-tungstenite", "tungstenite"]


def test_binances_rest_module_may_not_build_the_socket_stack() -> None:
    names = [*SOCKET_STACK, "polyoxide-core", "reqwest", "rustls", "tokio"]
    refused = [f"{n} ({SOCKETS})" for n in SOCKET_STACK]
    # `test-server` turns no module on, and a crate's defaults admit nothing.
    for features in (("usdm",), ("usdm", "test-server"), ("test-server",), ()):
        assert denied(BINANCE, names, features, BINANCE_TABLE) == refused, features
    # The streams admit it, and the HTTP stack and its TLS are the REST client's.
    for features in (("usdm-ws",), ("usdm-ws", "test-server"), ("usdm", "usdm-ws")):
        assert denied(BINANCE, names, features, BINANCE_TABLE) == [], features


def test_binances_rest_and_test_server_features_do_not_turn_on_the_streams() -> None:
    # The real table: a `usdm` or `test-server` that came to imply `usdm-ws`
    # would admit the socket stack, and `@usdm` would pass listing it.
    table = feature_table(BINANCE)
    for feature in ("usdm", "test-server"):
        assert "usdm-ws" not in enabled((feature,), table), feature
    assert "usdm" in enabled(("usdm-ws",), table)


# A trimmed stand-in for Kalshi's `[features]`: `predictions` is the REST module, and
# `predictions-ws`, which a later story adds, its order-book stream on the socket kit.
KALSHI_TABLE = {
    "default": ("predictions",),
    "predictions": ("dep:polyoxide-core", "polyoxide-venue/market", "dep:ring"),
    "predictions-ws": ("predictions", "dep:polyoxide-ws"),
}
KALSHI = "polyoxide-kalshi"
KALSHI_HTTP = "reqwest, polyoxide-core, governor and sha2 only with predictions"
KALSHI_NEVER = "alloy*, hmac and keyring are never built"
KALSHI_SOCKETS = "polyoxide-ws, tokio-tungstenite and tungstenite only with predictions-ws"


def test_kalshis_http_stack_comes_with_predictions_and_its_sockets_with_predictions_ws() -> None:
    names = [*HTTP_STACK, *SOCKET_STACK]
    # A crate's defaults admit nothing: a file names every feature it builds.
    assert denied(KALSHI, names, (), KALSHI_TABLE) == sorted(
        [f"{n} ({KALSHI_HTTP})" for n in HTTP_STACK]
        + [f"{n} ({KALSHI_SOCKETS})" for n in SOCKET_STACK])
    # The REST module builds the HTTP stack and no socket.
    assert denied(KALSHI, names, ("predictions",), KALSHI_TABLE) == [
        f"{n} ({KALSHI_SOCKETS})" for n in SOCKET_STACK]
    # The stream builds the socket kit on top of it.
    assert denied(KALSHI, names, ("predictions-ws",), KALSHI_TABLE) == []


def test_no_kalshi_file_may_build_the_signing_stack_or_the_keychain() -> None:
    names = ["alloy", "alloy-primitives", "hmac", "keyring", "ring"]
    never = [f"{n} ({KALSHI_NEVER})" for n in names[:4]]
    # Kalshi signs with `ring`, which no rule refuses.
    for features in ((), ("predictions",), ("predictions-ws",)):
        assert denied(KALSHI, names, features, KALSHI_TABLE) == never, features


UMBRELLA = "polyoxide"
UMBRELLA_HTTP = "reqwest, polyoxide-core, governor and sha2 only with an HTTP module"
UMBRELLA_SIGNING = "alloy* and hmac only with polymarket-clob or polymarket-relay"


def test_the_umbrellas_socket_passthroughs_may_not_build_the_http_stack() -> None:
    # The real table: a socket passthrough that came to turn on an HTTP module
    # of either venue would admit the HTTP stack, and its file would pass
    # listing it (5.5's G2).
    table = feature_table(UMBRELLA)
    names = [*HTTP_STACK, "alloy-primitives", "hmac", "keyring", "polyoxide-ws"]
    socket_only = [f"alloy-primitives ({UMBRELLA_SIGNING})", f"governor ({UMBRELLA_HTTP})",
                   f"hmac ({UMBRELLA_SIGNING})", f"keyring ({KEYCHAIN})",
                   f"polyoxide-core ({UMBRELLA_HTTP})", f"reqwest ({UMBRELLA_HTTP})",
                   f"sha2 ({UMBRELLA_HTTP})"]
    for features in (("polymarket-rtds",), ("polymarket-sports",),
                     ("polymarket-rtds", "polymarket-sports"), ()):
        assert denied(UMBRELLA, names, features, table) == socket_only, features
    # `keychain` admits the keychain and no HTTP stack.
    assert denied(UMBRELLA, HTTP_STACK, ("polymarket-rtds", "keychain"), table) == [
        f"{n} ({UMBRELLA_HTTP})" for n in HTTP_STACK]
    # Each HTTP module of either venue admits the HTTP stack, and only
    # Polymarket's clob and relay the signing stack.
    for features in (("polymarket-gamma",), ("polymarket-data",), ("polymarket-perps",),
                     ("polymarket-perps-ws",), ("binance-usdm",), ("binance-usdm-ws",),
                     ("binance",), ("polymarket-rtds", "binance-usdm")):
        assert denied(UMBRELLA, HTTP_STACK, features, table) == [], features
        assert denied(UMBRELLA, ["alloy", "hmac"], features, table) == [
            f"alloy ({UMBRELLA_SIGNING})", f"hmac ({UMBRELLA_SIGNING})"], features
    for features in (("polymarket",), ("polymarket-clob-ws",), ("polymarket-relay",)):
        assert denied(UMBRELLA, [*HTTP_STACK, "alloy", "hmac"], features, table) == [], features
    assert denied(UMBRELLA, ["keyring"], ("full",), table) == []


def test_only_a_crate_held_per_feature_is_built_without_its_defaults() -> None:
    assert no_defaults("polyoxide-polymarket") == ["--no-default-features"]
    assert no_defaults("polyoxide-binance") == ["--no-default-features"]
    # The umbrella's files name the passthroughs they build, not its defaults.
    assert no_defaults("polyoxide") == ["--no-default-features"]
    # The socket kit's bare file is the tree a consumer builds by default.
    assert no_defaults("polyoxide-ws") == []


def test_enabled_follows_features_and_not_dependencies() -> None:
    assert enabled(("clob-ws",), TABLE) == {"clob-ws", "clob"}
    assert enabled(("perps-ws",), TABLE) == {"perps-ws", "perps"}
    assert enabled(("keychain",), TABLE) == {"keychain"}
    assert enabled((), TABLE) == set()


if __name__ == "__main__":
    if sys.argv[1:] != ["--write"]:
        sys.exit("usage: python -m tests.test_dep_allowlists --write")
    write()
