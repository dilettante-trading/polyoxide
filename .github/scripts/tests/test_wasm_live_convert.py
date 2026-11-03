"""Unit tests for wasm_live_convert.py.

The browser job's runner prints a different report from nextest's, and `wasm_live.sh` turns it
into libtest-json for the classifier. The log below follows what wasm-bindgen-test-runner prints
for a failing test (the `probe_tag_line` run, captured from the real runner): a `---- name output
----` block whose lines are indented, a tag line just above an unindented-by-the-hook `panicked
at` line, a stack trace, and a `test result:` summary.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

from classify_failures import Verdict, classify
from wasm_live_convert import BINARY, ConvertError, events, main

SCRIPT = Path(__file__).parent.parent / "wasm_live_convert.py"

HEADER = """\
Set WASM_BINDGEN_TEST_TIMEOUT to 450 seconds...
Try find `webdriver.json` for configure browser's capabilities:
Ok
Loading Wasm module...
"""

STACK = """\
        Stack:

        Error
            at http://127.0.0.1:33323/wasm-bindgen-test:447:25
            at logError (http://127.0.0.1:33323/wasm-bindgen-test:976:18)

    JS exception that was thrown:
        RuntimeError: unreachable
            at browser_live.wasm.__rust_abort (http://127.0.0.1:33323/wasm-bindgen-test_bg.wasm:wasm-function[9814]:0x1a1f4d)
"""


def failed_block(name: str, tag: str | None, message: str) -> str:
    tag_line = f"        polyoxide-class={tag}\n" if tag else ""
    return (
        f"---- {name} output ----\n"
        "    error output:\n"
        f"{tag_line}"
        "        panicked at polyoxide-wasm-smoke/tests/browser_live.rs:90:5:\n"
        f"        {message}\n"
        "        \n"
        f"{STACK}\n"
    )


def log(results: list[tuple[str, str]], blocks: str, summary: str) -> str:
    lines = "".join(f"test {name} ... {outcome}\n" for name, outcome in results)
    return f"{HEADER}running {len(results)} tests\n{lines}\n{blocks}{summary}\n"


# Two failures: one tagged `transient` (a socket that opened and dropped), one with no tag. The
# runner spells a failure `FAIL`; the second uses libtest's `FAILED`.
TWO_FAILURES = log(
    [
        ("socket_perps", "FAIL"),
        ("get_clob", "ok"),
        ("get_fapi", "FAILED"),
        ("get_data", "ok"),
        ("skipped_one", "ignored"),
    ],
    "failures:\n\n"
    + failed_block("socket_perps", "transient", "perps socket: the socket dropped")
    + failed_block("get_fapi", None, "fapi GET /fapi/v1/ping: refused")
    + "failures:\n\n    socket_perps\n    get_fapi\n\n",
    "test result: FAILED. 2 passed; 2 failed; 1 ignored; 0 filtered out; finished in 28.22s\n",
)


def by_name(parsed: list[dict]) -> dict[str, dict]:
    return {event["name"]: event for event in parsed}


def test_each_failure_maps_to_its_own_block_and_tag():
    parsed = by_name(events(TWO_FAILURES))
    assert set(parsed) == {f"{BINARY}${n}" for n in ("socket_perps", "get_clob", "get_fapi", "get_data")}
    assert parsed[f"{BINARY}$get_clob"]["event"] == "ok"
    assert parsed[f"{BINARY}$get_data"]["event"] == "ok"

    perps = parsed[f"{BINARY}$socket_perps"]
    fapi = parsed[f"{BINARY}$get_fapi"]
    assert perps["event"] == fapi["event"] == "failed"
    assert "the socket dropped" in perps["stdout"] and "refused" not in perps["stdout"]
    assert "refused" in fapi["stdout"] and "the socket dropped" not in fapi["stdout"]
    # The tag line is dedented and sits just above the panic report in the classifier's shape.
    assert "\npolyoxide-class=transient\nthread 'main' panicked at " in "\n" + perps["stdout"]

    assert classify(perps["stdout"]) is Verdict.TRANSIENT
    assert classify(fapi["stdout"]) is Verdict.REAL


def test_the_second_failure_does_not_borrow_the_first_ones_tag():
    # Swap which test carries the tag: the tag follows its block, not the order of the log.
    swapped = TWO_FAILURES.replace("polyoxide-class=transient", "polyoxide-class=environmental")
    parsed = by_name(events(swapped))
    assert classify(parsed[f"{BINARY}$socket_perps"]["stdout"]) is Verdict.ENVIRONMENTAL
    assert classify(parsed[f"{BINARY}$get_fapi"]["stdout"]) is Verdict.REAL


def test_an_ignored_test_is_counted_but_not_reported():
    names = [event["name"] for event in events(TWO_FAILURES)]
    assert not any("skipped_one" in name for name in names)


def test_a_summary_count_above_the_results_parsed_is_refused():
    # The runner ran four tests and one never reported: a pass nobody saw.
    short = TWO_FAILURES.replace("test get_data ... ok\n", "")
    with pytest.raises(ConvertError, match="summary counts 5 tests but its log holds 4"):
        events(short)


def test_a_summary_count_below_the_results_parsed_is_refused():
    with pytest.raises(ConvertError, match="summary counts"):
        events(TWO_FAILURES.replace("2 passed; 2 failed", "1 passed; 2 failed"))


def test_a_log_with_no_summary_is_refused():
    cut = TWO_FAILURES.split("test result:")[0]
    with pytest.raises(ConvertError, match="no `test result:` summary"):
        events(cut)


def test_a_log_with_no_results_is_refused():
    with pytest.raises(ConvertError, match="no test results"):
        events(HEADER + "error: could not compile `polyoxide-wasm-smoke`\n")


def test_a_filtered_run_counts_what_it_ran():
    one = log(
        [("socket_perps", "ok")],
        "",
        "test result: ok. 1 passed; 0 failed; 0 ignored; 10 filtered out; finished in 21.00s\n",
    )
    assert [e["event"] for e in events(one)] == ["ok"]


def test_the_cli_writes_json_lines_and_exits_one_on_a_short_log(tmp_path: Path):
    good, bad, out = tmp_path / "good.log", tmp_path / "bad.log", tmp_path / "out.json"
    good.write_text(TWO_FAILURES)
    bad.write_text(TWO_FAILURES.replace("test get_data ... ok\n", ""))

    ok = subprocess.run([sys.executable, "-I", str(SCRIPT), str(good), str(out)], capture_output=True, text=True)
    assert ok.returncode == 0, ok.stderr
    assert [json.loads(line)["event"] for line in out.read_text().splitlines()] == ["failed", "ok", "failed", "ok"]

    out.unlink()
    short = subprocess.run([sys.executable, "-I", str(SCRIPT), str(bad), str(out)], capture_output=True, text=True)
    assert short.returncode == 1
    assert "summary counts 5 tests" in short.stderr
    assert not out.exists()


def test_main_rejects_a_wrong_argument_count(capsys):
    assert main([]) == 2
