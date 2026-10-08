"""Unit tests for scripts/publish_order.py.

The order, the absent filter and the release decision are tested on fixture
metadata, a stub HTTP getter and a stub command runner, so nothing here reaches
crates.io, origin or GitHub. `http_status` is tested against a local server. The
tests that read the real workspace, or build a temporary one, run
`cargo metadata --offline`, which needs a Rust toolchain but no network.
"""

from __future__ import annotations

import http.server
import importlib.util
import json
import subprocess
import sys
import threading
import tomllib
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]


def _load_publish_order():
    """`scripts/publish_order.py`, which lives outside this uv project."""
    spec = importlib.util.spec_from_file_location(
        "publish_order", REPO / "scripts" / "publish_order.py"
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


publish_order = _load_publish_order()
Pair = publish_order.Pair
PublishOrderError = publish_order.PublishOrderError

HEAD = "1" * 40
OTHER = "2" * 40
TAG_OBJECT = "3" * 40


def dep(name: str, kind: str | None = None, req: str = "^1.0.0") -> dict:
    """A workspace dependency as `cargo metadata` reports it: always with a path."""
    return {"name": name, "kind": kind, "req": req, "path": f"/ws/{name}"}


def registry_dep(name: str, kind: str | None = None) -> dict:
    """A crates.io dependency, which has no path."""
    return {"name": name, "kind": kind, "req": "^0.1.0", "path": None}


def package(name: str, *deps: dict, publish: list | None = None,
            version: str = "1.0.0", root: str = "/ws") -> dict:
    return {
        "name": name,
        "version": version,
        "id": f"path+file://{root}/{name}#{version}",
        "publish": publish,
        "manifest_path": f"{root}/{name}/Cargo.toml",
        "dependencies": list(deps),
    }


def metadata(*packages: dict) -> dict:
    return {"packages": list(packages), "workspace_members": [p["id"] for p in packages]}


def names(packages: list[dict]) -> list[str]:
    return [p["name"] for p in packages]


# --- order -------------------------------------------------------------------


def test_each_crate_follows_what_it_needs() -> None:
    """Dependencies win over the alphabetical tie-break, which would invert this."""
    doc = metadata(
        package("a-app", dep("b-clob"), dep("c-core")),
        package("b-clob", dep("c-core")),
        package("c-core"),
        package("d-free"),
    )
    assert names(publish_order.publish_order(doc)) == ["c-core", "b-clob", "a-app", "d-free"]


@pytest.mark.parametrize("kind", [None, "build", "dev"])
def test_normal_build_and_versioned_dev_dependencies_order(kind: str | None) -> None:
    """A versioned dev-dependency stays in the published manifest, so it orders too."""
    doc = metadata(package("a", dep("b", kind=kind)), package("b"))
    assert names(publish_order.publish_order(doc)) == ["b", "a"]


def test_path_only_dev_dependency_is_ignored() -> None:
    """cargo strips it when publishing, so `b` needing `a` is the only real edge."""
    doc = metadata(
        package("a", dep("b", kind="dev", req="*")),
        package("b", dep("a")),
    )
    assert names(publish_order.publish_order(doc)) == ["a", "b"]


def test_path_only_normal_dependency_still_orders() -> None:
    """Only dev-dependencies are stripped; a path-only normal one is still needed."""
    doc = metadata(package("a", dep("b", req="*")), package("b"))
    assert names(publish_order.publish_order(doc)) == ["b", "a"]


def test_a_registry_dependency_named_like_a_member_orders_nothing() -> None:
    """Without a path it is some other crate on crates.io, so `a -> b` is no edge.

    Counting it would make a cycle with `b -> a` and refuse the release.
    """
    doc = metadata(package("a", registry_dep("b")), package("b", dep("a")))
    assert names(publish_order.publish_order(doc)) == ["a", "b"]


def test_versioned_dev_dependency_cycle_fails_naming_its_crates() -> None:
    doc = metadata(
        package("alpha", dep("beta", kind="dev")),
        package("beta", dep("alpha")),
        package("downstream", dep("alpha")),
        package("free"),
    )
    with pytest.raises(PublishOrderError) as caught:
        publish_order.publish_order(doc)
    message = str(caught.value)
    assert "alpha -> beta -> alpha" in message
    # Blocked by the cycle, but not part of it.
    assert "downstream" not in message
    assert "free" not in message


def test_cycle_names_only_its_crates_when_a_blocked_crate_sorts_first() -> None:
    """The walk starts at `aaa-blocked`, outside the cycle, and must trim it off."""
    doc = metadata(
        package("aaa-blocked", dep("beta")),
        package("alpha", dep("beta", kind="dev")),
        package("beta", dep("alpha")),
        package("free"),
    )
    with pytest.raises(PublishOrderError) as caught:
        publish_order.publish_order(doc)
    message = str(caught.value)
    assert "beta -> alpha -> beta" in message
    assert "aaa-blocked" not in message
    assert "free" not in message


@pytest.mark.parametrize("kind", [None, "build", "dev"])
def test_a_needed_dependency_on_an_unpublishable_member_is_refused(kind: str | None) -> None:
    """crates.io would reject the upload partway through a release, so refuse it first.

    A versioned dev-dependency counts: it stays in the published manifest.
    """
    doc = metadata(package("app", dep("private", kind=kind)), package("private", publish=[]))
    with pytest.raises(publish_order.WorkspaceError) as caught:
        publish_order.publish_order(doc)
    message = str(caught.value)
    assert "app" in message
    assert "private" in message


def test_a_path_only_dev_dependency_on_an_unpublishable_member_is_allowed() -> None:
    """cargo strips it when publishing, so crates.io never needs `private`."""
    doc = metadata(package("app", dep("private", kind="dev", req="*")),
                   package("private", publish=[]))
    assert names(publish_order.publish_order(doc)) == ["app"]


def test_unpublishable_members_are_excluded() -> None:
    """`publish = false` reads back as `[]`, and another registry is not crates.io."""
    doc = metadata(
        package("py", dep("core"), publish=[]),
        package("core"),
        package("elsewhere", publish=["other"]),
        package("registry-only", publish=["crates-io"]),
    )
    assert names(publish_order.publish_order(doc)) == ["core", "registry-only"]


def test_non_members_are_excluded() -> None:
    doc = metadata(package("core"))
    doc["packages"].append(package("not-a-member"))
    assert names(publish_order.publish_order(doc)) == ["core"]


# --- tombstones --------------------------------------------------------------


def test_tombstones_follow_every_workspace_crate() -> None:
    """Even a tombstone whose name sorts first, and one needed by nothing."""
    workspace = metadata(package("venue", dep("core")), package("core"))
    tombstone = metadata(package("aaa-old", version="9.9.9", root="/ws/tombstones"))
    pairs = publish_order.planned(workspace, [tombstone])
    assert [p.name for p in pairs] == ["core", "venue", "aaa-old"]
    assert pairs[-1] == Pair("aaa-old", "9.9.9", "/ws/tombstones/aaa-old/Cargo.toml")


def _write_crate(directory: Path, name: str, version: str) -> None:
    (directory / "src").mkdir(parents=True)
    (directory / "src" / "lib.rs").write_text("")
    (directory / "Cargo.toml").write_text(
        f'[package]\nname = "{name}"\nversion = "{version}"\nedition = "2021"\n'
    )


def _write_workspace(root: Path, exclude: bool) -> None:
    _write_crate(root / "venue", "venue", "1.0.0")
    _write_crate(root / "tombstones" / "zeta-old", "zeta-old", "0.9.1")
    _write_crate(root / "tombstones" / "alpha-old", "alpha-old", "0.4.2")
    excluded = 'exclude = ["tombstones"]\n' if exclude else ""
    (root / "Cargo.toml").write_text(
        f'[workspace]\nmembers = ["venue"]\n{excluded}resolver = "2"\n'
    )


def test_tombstone_metadata_reads_a_real_tree(tmp_path: Path) -> None:
    _write_workspace(tmp_path, exclude=True)
    tombstones = publish_order.tombstone_metadata(tmp_path)
    workspace = publish_order.cargo_metadata(tmp_path / "Cargo.toml")
    pairs = publish_order.planned(workspace, tombstones)
    assert [(p.name, p.version) for p in pairs] == [
        ("venue", "1.0.0"), ("alpha-old", "0.4.2"), ("zeta-old", "0.9.1"),
    ]
    assert Path(pairs[1].manifest) == (tmp_path / "tombstones/alpha-old/Cargo.toml").resolve()


def test_a_tombstone_the_workspace_does_not_exclude_is_an_error(tmp_path: Path) -> None:
    """cargo refuses a package that sits inside a workspace without belonging to it."""
    _write_workspace(tmp_path, exclude=False)
    with pytest.raises(PublishOrderError, match="cargo metadata"):
        publish_order.tombstone_metadata(tmp_path)


def test_the_real_workspace_excludes_tombstones() -> None:
    """Without it, the first tombstone would make every `list` fail."""
    workspace = tomllib.loads((REPO / "Cargo.toml").read_text())["workspace"]
    assert "tombstones" in workspace.get("exclude", [])


# --- absent filter -----------------------------------------------------------


class StubCratesIo:
    """Answers from a fixed table and records every request and pause."""

    def __init__(self, crates: dict[str, set[str]], fail: int | None = None) -> None:
        self.crates = crates
        self.fail = fail
        self.requests: list[tuple[str, dict]] = []
        self.sleeps: list[float] = []
        self.events: list[str] = []

    def get(self, url: str, headers: dict) -> int:
        self.requests.append((url, headers))
        self.events.append("get")
        if self.fail is not None:
            return self.fail
        name, _, version = url.removeprefix(publish_order.CRATES_IO + "/").partition("/")
        if name not in self.crates:
            return 404
        return 200 if not version or version in self.crates[name] else 404

    def sleep(self, seconds: float) -> None:
        self.sleeps.append(seconds)
        self.events.append("sleep")


PAIRS = [
    Pair("published", "1.0.0", "/ws/published/Cargo.toml"),
    Pair("bumped", "1.1.0", "/ws/bumped/Cargo.toml"),
    Pair("brand-new", "1.0.0", "/ws/brand-new/Cargo.toml"),
]


def test_absent_filter_keeps_only_missing_pairs_and_names_new_crates() -> None:
    stub = StubCratesIo({"published": {"1.0.0"}, "bumped": {"1.0.0"}})
    missing, new_names = publish_order.absent(PAIRS, stub.get, stub.sleep)
    assert missing == PAIRS[1:]
    assert new_names == ["brand-new"]


def test_absent_filter_follows_the_crawler_policy() -> None:
    """A User-Agent naming the repository, and a second's pause between requests."""
    stub = StubCratesIo({"published": {"1.0.0"}, "bumped": {"1.0.0"}})
    publish_order.absent(PAIRS, stub.get, stub.sleep)
    # One request for the present pair, two for each absent one.
    assert len(stub.requests) == 5
    for _, headers in stub.requests:
        assert "https://github.com/dilettante-trading/polyoxide" in headers["User-Agent"]
    assert stub.events == ["get"] + ["sleep", "get"] * 4
    assert all(seconds >= 1.0 for seconds in stub.sleeps)


@pytest.mark.parametrize("status", [403, 429, 500])
def test_absent_filter_stops_on_an_unexpected_status(status: int) -> None:
    """Neither 200 nor 404 says whether the pair exists."""
    stub = StubCratesIo({}, fail=status)
    with pytest.raises(PublishOrderError, match=str(status)):
        publish_order.absent(PAIRS, stub.get, stub.sleep)


def _new_crates(count: int) -> dict:
    return metadata(*(package(f"new-{i}") for i in range(count)))


def test_five_new_names_are_allowed() -> None:
    stub = StubCratesIo({})
    pairs = publish_order.pending(_new_crates(5), [], stub.get, stub.sleep, max_new_names=5)
    assert len(pairs) == 5


def test_more_than_five_new_names_exit_3() -> None:
    """3, not argparse's 2, so finish_release.sh can tell this from a usage error."""
    stub = StubCratesIo({})
    with pytest.raises(publish_order.TooManyNewNames) as caught:
        publish_order.pending(_new_crates(6), [], stub.get, stub.sleep, max_new_names=5)
    assert caught.value.exit_code == 3
    message = str(caught.value)
    assert "new-5" in message
    assert "rate-limits new crate registrations" in message
    assert "Split the release" in message


def test_new_versions_of_known_names_are_not_new_names() -> None:
    stub = StubCratesIo({f"new-{i}": {"0.9.0"} for i in range(6)})
    pairs = publish_order.pending(_new_crates(6), [], stub.get, stub.sleep, max_new_names=5)
    assert len(pairs) == 6


# --- http_status -------------------------------------------------------------


class _Handler(http.server.BaseHTTPRequestHandler):
    """`/<status>/...` answers with that status; `/drop/...` closes without answering."""

    def do_GET(self) -> None:  # noqa: N802 - the stdlib's name
        self.server.user_agents.append(self.headers.get("User-Agent"))
        first = self.path.strip("/").split("/")[0]
        if first == "drop":
            self.close_connection = True
            return
        status = int(first)
        self.send_response(status)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"{}")

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002
        pass


@pytest.fixture
def server(monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setenv("no_proxy", "*")
    monkeypatch.setenv("NO_PROXY", "*")
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    httpd.user_agents = []
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    yield httpd
    httpd.shutdown()
    httpd.server_close()


def _url(httpd: http.server.ThreadingHTTPServer, path: str) -> str:
    return f"http://127.0.0.1:{httpd.server_address[1]}/{path}"


@pytest.mark.parametrize("status", [200, 404, 429])
def test_http_status_returns_the_status(server, status: int) -> None:
    headers = {"User-Agent": publish_order.USER_AGENT}
    assert publish_order.http_status(_url(server, str(status)), headers) == status
    assert server.user_agents == [publish_order.USER_AGENT]


def test_http_status_turns_a_dropped_connection_into_an_error(server) -> None:
    """A reset or a read timeout must be `::error::`, never a traceback."""
    with pytest.raises(PublishOrderError, match="could not reach crates.io"):
        publish_order.http_status(_url(server, "drop"), {"User-Agent": "x"})


def test_http_status_turns_a_refused_connection_into_an_error(server) -> None:
    port = server.server_address[1]
    server.shutdown()
    server.server_close()
    with pytest.raises(PublishOrderError, match="could not reach crates.io"):
        publish_order.http_status(f"http://127.0.0.1:{port}/200", {"User-Agent": "x"})


def test_the_absent_filter_sends_its_user_agent_on_the_wire(
        server, monkeypatch: pytest.MonkeyPatch) -> None:
    """urllib renames headers; the server must still see the repository's agent."""
    monkeypatch.setattr(publish_order, "CRATES_IO", _url(server, "200"))
    missing, _ = publish_order.absent([PAIRS[0]], publish_order.http_status, lambda _: None)
    assert missing == []
    assert server.user_agents == [publish_order.USER_AGENT]


# --- version -----------------------------------------------------------------


def test_version_is_the_publishable_members_version() -> None:
    doc = metadata(package("a"), package("b"), package("py", publish=[], version="0.0.1"))
    assert publish_order.workspace_version(doc) == "1.0.0"


def test_version_fails_when_publishable_members_disagree() -> None:
    doc = metadata(package("a"), package("b", version="1.0.1"))
    with pytest.raises(PublishOrderError, match="a 1.0.0, b 1.0.1"):
        publish_order.workspace_version(doc)


# --- tag-sha and decide's lookups --------------------------------------------


class StubRunner:
    """Answers each command by its leading words, recording every call."""

    def __init__(self, answers: dict[tuple[str, ...], tuple[int, str, str]]) -> None:
        self.answers = answers
        self.calls: list[list[str]] = []

    def __call__(self, command: list[str]) -> subprocess.CompletedProcess:
        self.calls.append(command)
        for prefix, (code, out, err) in self.answers.items():
            if tuple(command[: len(prefix)]) == prefix:
                return subprocess.CompletedProcess(command, code, out, err)
        raise AssertionError(f"unexpected command {command}")

    def ran(self, *prefix: str) -> bool:
        return any(tuple(c[: len(prefix)]) == prefix for c in self.calls)


LS_REMOTE = ("git", "ls-remote")


def _ls_remote(*lines: tuple[str, str]) -> StubRunner:
    return StubRunner({LS_REMOTE: (0, "".join(f"{sha}\t{ref}\n" for sha, ref in lines), "")})


def test_tag_sha_prefers_the_peeled_commit() -> None:
    """An annotated tag's direct line names the tag object, not the commit."""
    run = _ls_remote((TAG_OBJECT, "refs/tags/v1.0.0"), (HEAD, "refs/tags/v1.0.0^{}"))
    assert publish_order.tag_sha("1.0.0", run) == HEAD
    assert run.calls == [["git", "ls-remote", "origin", "refs/tags/v1.0.0",
                          "refs/tags/v1.0.0^{}"]]


def test_tag_sha_falls_back_to_a_lightweight_tag() -> None:
    run = _ls_remote((HEAD, "refs/tags/v1.0.0"))
    assert publish_order.tag_sha("1.0.0", run) == HEAD


def test_tag_sha_is_none_without_a_tag() -> None:
    assert publish_order.tag_sha("1.0.0", _ls_remote()) is None


def test_tag_sha_ignores_other_refs() -> None:
    run = _ls_remote((OTHER, "refs/tags/archive/refs/tags/v1.0.0"))
    assert publish_order.tag_sha("1.0.0", run) is None


def test_a_failed_ls_remote_raises_rather_than_reading_as_no_tag() -> None:
    run = StubRunner({LS_REMOTE: (128, "", "fatal: unable to access origin")})
    with pytest.raises(PublishOrderError, match="unable to access origin"):
        publish_order.tag_sha("1.0.0", run)


PARENT_MANIFEST = '[workspace.package]\nversion = "0.9.0"\n'


def test_parent_version_reads_the_parent_manifest() -> None:
    run = StubRunner({
        ("git", "rev-list"): (0, f"{HEAD} {OTHER}\n", ""),
        ("git", "show"): (0, PARENT_MANIFEST, ""),
    })
    assert publish_order.parent_version(HEAD, run) == "0.9.0"
    assert ["git", "show", f"{HEAD}^:Cargo.toml"] in run.calls


def test_no_parent_gives_none() -> None:
    run = StubRunner({("git", "rev-list"): (0, f"{HEAD}\n", "")})
    assert publish_order.parent_version(HEAD, run) is None
    assert not run.ran("git", "show")


@pytest.mark.parametrize("failing", [("git", "rev-list"), ("git", "show")])
def test_a_failed_parent_lookup_raises(failing: tuple[str, ...]) -> None:
    answers = {
        ("git", "rev-list"): (0, f"{HEAD} {OTHER}\n", ""),
        ("git", "show"): (0, PARENT_MANIFEST, ""),
    }
    answers[failing] = (128, "", "fatal: bad object")
    with pytest.raises(PublishOrderError, match="bad object"):
        publish_order.parent_version(HEAD, StubRunner(answers))


GH_RELEASE = ("gh", "release", "view")


def test_release_exists_on_exit_0() -> None:
    run = StubRunner({GH_RELEASE: (0, '{"tagName":"v1.0.0"}', "")})
    assert publish_order.release_exists("1.0.0", run) is True


def test_release_is_absent_on_exit_1_with_not_found() -> None:
    run = StubRunner({GH_RELEASE: (1, "", "release not found\n")})
    assert publish_order.release_exists("1.0.0", run) is False


@pytest.mark.parametrize(("code", "stderr"), [
    # gh exits 1 for a failed request as well, so exit 1 alone is not "absent".
    (1, "HTTP 401: Bad credentials"),
    (4, "To get started with GitHub CLI, please run:  gh auth login"),
    (2, ""),
])
def test_any_other_gh_release_outcome_raises(code: int, stderr: str) -> None:
    run = StubRunner({GH_RELEASE: (code, "", stderr)})
    with pytest.raises(PublishOrderError, match=f"exited {code}"):
        publish_order.release_exists("1.0.0", run)


GH_RUNS = ("gh", "run", "list")


@pytest.mark.parametrize(("runs", "passed"), [
    ([{"status": "completed", "conclusion": "success"}], True),
    ([{"status": "completed", "conclusion": "failure"},
      {"status": "completed", "conclusion": "success"}], True),
    ([], False),
    ([{"status": "in_progress", "conclusion": ""}], False),
    ([{"status": "completed", "conclusion": "failure"}], False),
])
def test_ci_passed_needs_a_completed_success(runs: list[dict], passed: bool) -> None:
    run = StubRunner({GH_RUNS: (0, json.dumps(runs), "")})
    assert publish_order.ci_passed(HEAD, run) is passed
    assert run.calls == [["gh", "run", "list", "--workflow", "CI", "--commit", HEAD,
                          "--json", "conclusion,status"]]


def test_a_failed_run_list_raises() -> None:
    run = StubRunner({GH_RUNS: (1, "", "could not find any workflows named CI")})
    with pytest.raises(PublishOrderError, match="could not find any workflows"):
        publish_order.ci_passed(HEAD, run)


@pytest.mark.parametrize(("runs", "code"), [
    ([{"status": "completed", "conclusion": "success"}], 0),
    ([{"status": "completed", "conclusion": "failure"}], 1),
])
def test_the_ci_passed_command_exits_0_only_on_success(
        capsys: pytest.CaptureFixture[str], runs: list[dict], code: int) -> None:
    run = StubRunner({GH_RUNS: (0, json.dumps(runs), "")})
    assert publish_order.main(["ci-passed", HEAD], run=run) == code
    out, err = capsys.readouterr()
    assert out == ""
    assert err == ("" if code == 0 else f"::error::CI has not passed on {HEAD}\n")


def test_a_stalled_command_is_an_error(monkeypatch: pytest.MonkeyPatch) -> None:
    """A stalled `git ls-remote` or `gh` must not hold the release job for hours."""
    def stall(command, **kwargs):
        assert kwargs["timeout"] == publish_order.COMMAND_TIMEOUT
        raise subprocess.TimeoutExpired(command, kwargs["timeout"])

    monkeypatch.setattr(publish_order.subprocess, "run", stall)
    with pytest.raises(PublishOrderError, match="did not finish"):
        publish_order.run_command(["git", "ls-remote", "origin"])


# --- decide ------------------------------------------------------------------


def _decide(*, current: str = "1.0.0", parent: str | None = "0.9.0", tag: str | None = None,
            dispatch: bool = False, released: bool = False, ci_ok: bool | None = None):
    if dispatch and ci_ok is None:
        ci_ok = True
    return publish_order.decide(current, parent, tag, HEAD, dispatch, released, ci_ok)


@pytest.mark.parametrize("tag", [None, HEAD, OTHER])
def test_row_1_a_manual_run_needs_ci_to_have_passed(tag: str | None) -> None:
    """`--no-verify` publishes what CI built, so a manual run must not skip CI."""
    with pytest.raises(PublishOrderError) as caught:
        _decide(tag=tag, dispatch=True, ci_ok=False)
    assert str(caught.value) == f"CI has not passed on {HEAD}"


@pytest.mark.parametrize(("parent", "dispatch"), [
    ("0.9.0", False),
    (None, False),
    ("0.9.0", True),
    ("1.0.0", True),
    # A fix-forward or an empty commit after a red version-bump commit: the bump
    # never released, so this push releases it.
    ("1.0.0", False),
])
def test_row_2_an_untagged_version_releases(parent: str | None, dispatch: bool) -> None:
    decision = _decide(parent=parent, tag=None, dispatch=dispatch)
    assert decision.result == "release"
    assert decision.log.startswith("::notice::")


@pytest.mark.parametrize("dispatch", [False, True])
def test_row_3_a_released_tag_at_this_commit_skips(dispatch: bool) -> None:
    """A re-run of a finished release does nothing."""
    decision = _decide(tag=HEAD, released=True, dispatch=dispatch)
    assert decision.result == "skip"


@pytest.mark.parametrize(("parent", "dispatch"), [("0.9.0", False), ("1.0.0", True)])
def test_row_4_a_tag_at_this_commit_without_a_release_resumes(
        parent: str, dispatch: bool) -> None:
    decision = _decide(parent=parent, tag=HEAD, released=False, dispatch=dispatch)
    assert decision.result == "release"
    assert "resumes" in decision.log


def test_row_5_a_push_that_keeps_a_released_version_skips_quietly() -> None:
    """Every ordinary push to main lands here, so it logs a plain line, no annotation."""
    decision = _decide(parent="1.0.0", tag=OTHER)
    assert decision.result == "skip"
    assert not decision.log.startswith("::")


@pytest.mark.parametrize(("current", "parent", "dispatch"), [
    ("1.0.0", "1.1.0", False),
    ("1.0.0", "1.1.0", True),
    # Numeric, not text: "0.10.0" sorts before "0.9.0" as a string.
    ("0.9.0", "0.10.0", False),
    ("1.0.0", "1.0.1-rc.1", False),
])
def test_row_6_a_version_that_went_backwards_skips_with_a_warning(
        current: str, parent: str, dispatch: bool) -> None:
    decision = _decide(current=current, parent=parent, tag=OTHER, dispatch=dispatch)
    assert decision.result == "skip"
    assert decision.log.startswith("::warning::")


@pytest.mark.parametrize(("current", "parent", "dispatch"), [
    ("1.0.0", "0.9.0", False),
    ("0.10.0", "0.9.0", False),
    ("1.0.0", "1.0.0-rc.1", False),
    ("1.0.0", None, False),
    # A manual run skips the version-bump test, so an unchanged version is no excuse.
    ("1.0.0", "1.0.0", True),
])
def test_row_7_a_tag_at_another_commit_fails(
        current: str, parent: str | None, dispatch: bool) -> None:
    with pytest.raises(PublishOrderError) as caught:
        _decide(current=current, parent=parent, tag=OTHER, dispatch=dispatch)
    assert str(caught.value) == f"v{current} already tagged at {OTHER}; bump the version"


# --- decide from the command line --------------------------------------------


def _decide_runner(*, tag_lines: str = "", parent: bool = True,
                   release: tuple = (1, "", "release not found"),
                   ls_remote: tuple | None = None,
                   runs: list[dict] | None = None) -> StubRunner:
    """The lookups `decide` makes, for a workspace at 1.0.0 whose parent is at 0.9.0."""
    return StubRunner({
        ("cargo", "metadata"): (0, json.dumps(metadata(package("a"), package("b"))), ""),
        ("git", "rev-list"): (0, f"{HEAD} {OTHER}\n" if parent else f"{HEAD}\n", ""),
        ("git", "show"): (0, PARENT_MANIFEST, ""),
        LS_REMOTE: ls_remote or (0, tag_lines, ""),
        GH_RELEASE: release,
        GH_RUNS: (0, json.dumps(runs if runs is not None else []), ""),
    })


def test_decide_releases_an_untagged_bump(capsys: pytest.CaptureFixture[str]) -> None:
    run = _decide_runner()
    assert publish_order.main(["decide", "--head-sha", HEAD], run=run) == 0
    out, err = capsys.readouterr()
    assert out == "release\n"
    assert err.startswith("::notice::")
    # CI's runs matter only to a manual run; a workflow_run run follows a green CI.
    assert not run.ran(*GH_RUNS)


@pytest.mark.parametrize("tag_lines", [
    f"{TAG_OBJECT}\trefs/tags/v1.0.0\n{OTHER}\trefs/tags/v1.0.0^{{}}\n",
    f"{OTHER}\trefs/tags/v1.0.0\n",
], ids=["annotated", "lightweight"])
def test_decide_fails_on_a_bump_to_a_version_tagged_elsewhere(
        capsys: pytest.CaptureFixture[str], tag_lines: str) -> None:
    run = _decide_runner(tag_lines=tag_lines)
    assert publish_order.main(["decide", "--head-sha", HEAD], run=run) == 1
    out, err = capsys.readouterr()
    assert out == ""
    assert err == f"::error::v1.0.0 already tagged at {OTHER}; bump the version\n"


@pytest.mark.parametrize("failure", ["ls-remote", "gh"])
def test_decide_fails_when_a_lookup_fails(
        capsys: pytest.CaptureFixture[str], failure: str) -> None:
    """A lookup that failed must never read as "no tag" or "no release"."""
    if failure == "ls-remote":
        run = _decide_runner(ls_remote=(128, "", "fatal: could not read from remote"))
    else:
        run = _decide_runner(release=(1, "", "HTTP 502: Bad Gateway"))
    assert publish_order.main(["decide", "--head-sha", HEAD], run=run) == 1
    out, err = capsys.readouterr()
    assert out == ""
    assert err.startswith("::error::")


def test_decide_skips_a_rerun_of_a_finished_release(capsys: pytest.CaptureFixture[str]) -> None:
    run = _decide_runner(tag_lines=f"{HEAD}\trefs/tags/v1.0.0^{{}}\n", release=(0, "{}", ""))
    assert publish_order.main(["decide", "--head-sha", HEAD], run=run) == 0
    assert capsys.readouterr().out == "skip\n"


def test_decide_on_dispatch_asks_whether_ci_passed(capsys: pytest.CaptureFixture[str]) -> None:
    run = _decide_runner(runs=[{"status": "in_progress", "conclusion": ""}])
    assert publish_order.main(["decide", "--head-sha", HEAD, "--dispatch"], run=run) == 1
    out, err = capsys.readouterr()
    assert out == ""
    assert err == f"::error::CI has not passed on {HEAD}\n"


def test_decide_on_a_root_commit(capsys: pytest.CaptureFixture[str]) -> None:
    run = _decide_runner(parent=False)
    assert publish_order.main(["decide", "--head-sha", HEAD], run=run) == 0
    assert capsys.readouterr().out == "release\n"
    assert not run.ran("git", "show")


def test_tag_sha_prints_the_commit_or_nothing(capsys: pytest.CaptureFixture[str]) -> None:
    run = _ls_remote((TAG_OBJECT, "refs/tags/v1.0.0"), (HEAD, "refs/tags/v1.0.0^{}"))
    assert publish_order.main(["tag-sha", "1.0.0"], run=run) == 0
    assert capsys.readouterr().out == f"{HEAD}\n"
    assert publish_order.main(["tag-sha", "1.0.0"], run=_ls_remote()) == 0
    assert capsys.readouterr().out == ""


def test_tag_sha_fails_loudly(capsys: pytest.CaptureFixture[str]) -> None:
    run = StubRunner({LS_REMOTE: (128, "", "fatal: could not read from remote")})
    assert publish_order.main(["tag-sha", "1.0.0"], run=run) == 1
    out, err = capsys.readouterr()
    assert out == ""
    assert err.startswith("::error::")


# --- the real workspace ------------------------------------------------------


def test_real_workspace_order() -> None:
    order = names(publish_order.publish_order(publish_order.workspace_metadata()))
    assert order[0] == "polyoxide-core"
    assert order.index("polyoxide") > order.index("polyoxide-clob")
    assert "polyoxide-cli" in order
    assert "polyoxide-py" not in order


@pytest.fixture
def no_tombstones(monkeypatch: pytest.MonkeyPatch) -> None:
    """The first tombstone must not break tests about the workspace's own pairs."""
    monkeypatch.setattr(publish_order, "tombstone_metadata", lambda *_: [])


@pytest.mark.usefixtures("no_tombstones")
def test_list_prints_only_what_crates_io_lacks(capsys: pytest.CaptureFixture[str]) -> None:
    """Every name known at the current version except the CLI's."""
    version = publish_order.workspace_version()
    order = names(publish_order.publish_order(publish_order.workspace_metadata()))
    stub = StubCratesIo({name: {version} for name in order if name != "polyoxide-cli"})
    assert publish_order.main(["list"], get=stub.get, sleep=stub.sleep) == 0
    out, _ = capsys.readouterr()
    manifest = REPO / "polyoxide-cli" / "Cargo.toml"
    assert out == f"polyoxide-cli {version} {manifest}\n"


@pytest.mark.usefixtures("no_tombstones")
def test_list_refuses_a_workspace_of_new_names(capsys: pytest.CaptureFixture[str]) -> None:
    stub = StubCratesIo({})
    assert publish_order.main(["--max-new-names", "5"], get=stub.get, sleep=stub.sleep) == 3
    out, err = capsys.readouterr()
    assert out == ""
    assert err.startswith("::error::")


@pytest.mark.usefixtures("no_tombstones")
@pytest.mark.parametrize("answer", [
    (0, json.dumps(metadata(package("a", dep("b", kind="dev")), package("b", dep("a")))), ""),
    (0, json.dumps(metadata(package("a", dep("b")), package("b", publish=[]))), ""),
    (101, "", "error: failed to parse manifest"),
], ids=["cycle", "unpublishable", "cargo-metadata"])
def test_list_exits_4_when_the_workspace_cannot_be_published(
        capsys: pytest.CaptureFixture[str], answer: tuple) -> None:
    """Retrying cannot fix these, so finish_release.sh must not mistake them for 1."""
    stub = StubCratesIo({})
    run = StubRunner({("cargo", "metadata"): answer})
    assert publish_order.main(["list"], get=stub.get, sleep=stub.sleep, run=run) == 4
    out, err = capsys.readouterr()
    assert out == ""
    assert err.startswith("::error::")
    assert stub.requests == []


@pytest.mark.usefixtures("no_tombstones")
def test_list_exits_1_when_crates_io_fails(capsys: pytest.CaptureFixture[str]) -> None:
    """A crates.io failure may pass, so it keeps the retryable code."""
    stub = StubCratesIo({}, fail=503)
    run = StubRunner({("cargo", "metadata"): (0, json.dumps(metadata(package("a"))), "")})
    assert publish_order.main(["list"], get=stub.get, sleep=stub.sleep, run=run) == 1
    assert capsys.readouterr().err.startswith("::error::crates.io answered 503")
