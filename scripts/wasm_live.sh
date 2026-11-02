#!/usr/bin/env bash
# The nightly's browser job (spec D14, D17). It runs polyoxide-wasm-smoke's `browser_live` in
# headless Chrome through wasm-bindgen-test-runner and writes what the nightly classifier reads:
# libtest-json, since the classifier was written for nextest. A test that failed carries the
# runner's output for it, with the `polyoxide-class=<tag>` line the tags print.
#
#   target/wasm-live/first-pass.json   every test, as ok or failed
#   target/wasm-live/retry.json        the tests the classifier called transient, run once more
#
# Before the real run it runs a probe: a test that fails with a `transient` tag. The classifier
# must read that tag from the runner's output, or this script exits 1, since a job that cannot
# read its tags would file every failure as `real`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

out=target/wasm-live
binary='polyoxide-wasm-smoke::browser_live'
mkdir -p "$out"
rm -rf "$out"/*.log "$out"/*.json "$out"/probe "$out"/first-pass

for tool in wasm-bindgen wasm-bindgen-test-runner; do
  if ! command -v "$tool" >/dev/null; then
    echo "wasm_live: $tool is not on PATH; run inside \`nix develop\` or install wasm-bindgen-cli" >&2
    exit 1
  fi
done

# The runner and the wasm-bindgen crate must be the same version.
locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [ "$locked" != "$installed" ]; then
  echo "wasm_live: wasm-bindgen-cli is $installed but Cargo.lock pins wasm-bindgen $locked; they must match" >&2
  exit 1
fi

# The runner takes its driver from CHROMEDRIVER. A GitHub runner has it beside CHROMEWEBDRIVER.
if [ -z "${CHROMEDRIVER:-}" ]; then
  if command -v chromedriver >/dev/null; then
    CHROMEDRIVER=$(command -v chromedriver)
  elif [ -x "${CHROMEWEBDRIVER:-}/chromedriver" ]; then
    CHROMEDRIVER="$CHROMEWEBDRIVER/chromedriver"
  else
    echo "wasm_live: no chromedriver on PATH, in CHROMEDRIVER or in CHROMEWEBDRIVER" >&2
    exit 1
  fi
  export CHROMEDRIVER
fi

# A browser cannot tell Binance's region block (a 451, which the nightly's US runners get) from any
# other failed fetch or upgrade, and nobody has measured whether that 451 carries the CORS headers
# a page needs to read it. So read the REST host's status here, natively, once and without a retry:
# on a 451 the suite is built with POLYOXIDE_LIVE_BINANCE_STATUS=451, which fails the three
# Binance checks `environmental` at once, without calling Binance from the browser. Any other
# status, or a curl failure, leaves the run unchanged. WASM_LIVE_BINANCE_STATUS=<status> stands in
# for the read (to prove the 451 path from an address Binance accepts).
binance_status=${WASM_LIVE_BINANCE_STATUS:-}
if [ -z "$binance_status" ]; then
  binance_status=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 \
    https://fapi.binance.com/fapi/v1/ping || true)
fi
case "$binance_status" in
  '' | 000) echo "wasm_live: Binance's REST host could not be read natively (no curl, or no answer)" >&2 ;;
  *) echo "wasm_live: Binance's REST host answered $binance_status natively" >&2 ;;
esac
if [ "$binance_status" = 451 ]; then
  export POLYOXIDE_LIVE_BINANCE_STATUS=451
  echo "wasm_live: a 451, so the Binance checks fail environmental without a browser call" >&2
fi

# run <log> [filter]: the live tests, whose output goes to <log> and to stderr. A failing test
# is the classifier's business, so a failing run is not this script's.
run() {
  local log=$1
  shift
  RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
  CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  WASM_BINDGEN_TEST_TIMEOUT=800 \
    cargo test -p polyoxide-wasm-smoke --target wasm32-unknown-unknown \
      --features live --test browser_live -- "$@" >"$log" 2>&1 </dev/null || true
  cat "$log" >&2
}

# convert <log> <json>: the runner's report as libtest-json events. A failed test's block is
# dedented, and the panic report the hook leaves is given the `thread '...' panicked at` shape the
# classifier looks for, so the tag line stays just above it.
convert() {
  python3 -I - "$1" "$2" <<'PY'
import json
import re
import sys

log, out = sys.argv[1], sys.argv[2]
BINARY = "polyoxide-wasm-smoke::browser_live"
# The runner spells a failure `FAIL`; libtest's `FAILED` is read too.
RESULT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|FAIL|ignored)\s*$")
HEADING = re.compile(r"^---- (\S+) output ----\s*$")
PANIC = re.compile(r"^panicked at ")

lines = open(log, encoding="utf-8", errors="replace").read().splitlines()

results = {}
for line in lines:
    found = RESULT.match(line.strip())
    if found:
        results[found[1]] = found[2]

blocks, current = {}, None
for line in lines:
    heading = HEADING.match(line.strip())
    if heading:
        current = heading[1]
        blocks[current] = []
        continue
    if current is None:
        continue
    text = line.strip()
    if text.startswith(("failures:", "test result:")):
        current = None
        continue
    if PANIC.match(text):
        text = "thread 'main' " + text
    blocks[current].append(text)

if not results:
    sys.exit("wasm_live: the runner printed no test results; the build or the runner failed")

with open(out, "w", encoding="utf-8") as sink:
    for name, outcome in results.items():
        if outcome == "ignored":
            continue
        event = {
            "type": "test",
            "name": f"{BINARY}${name.rsplit('::', 1)[-1]}",
            "event": "ok" if outcome == "ok" else "failed",
        }
        if outcome != "ok":
            event["stdout"] = "\n".join(blocks.get(name, [])) + "\n"
        sink.write(json.dumps(event) + "\n")
PY
}

# classify <json> <dir>: the classifier's own first pass, which writes retry-tests.txt and
# real-failures.txt into <dir>.
classify() {
  python3 -I .github/scripts/classify_failures.py classify --input "$1" --output-dir "$2"
}

POLYOXIDE_LIVE_PROBE=1 run "$out/probe.log" probe_tag_line
convert "$out/probe.log" "$out/probe.json"
classify "$out/probe.json" "$out/probe"
if ! grep -qxF "$binary\$probe_tag_line" "$out/probe/retry-tests.txt"; then
  echo "wasm_live: the probe's tag line did not reach the classifier; the runner's output is above" >&2
  exit 1
fi
echo "wasm_live: the probe's tag line reached the classifier" >&2

run "$out/first-pass.log"
convert "$out/first-pass.log" "$out/first-pass.json"
classify "$out/first-pass.json" "$out/first-pass"

# One more run of what the classifier calls transient, as the native suites' retry pass does.
: >"$out/retry.json"
if [ -s "$out/first-pass/retry-tests.txt" ]; then
  n=0
  while IFS= read -r name; do
    n=$((n + 1))
    run "$out/retry-$n.log" "${name#*\$}"
    convert "$out/retry-$n.log" "$out/retry-$n.json"
    cat "$out/retry-$n.json" >>"$out/retry.json"
  done <"$out/first-pass/retry-tests.txt"
fi

echo "wasm_live: wrote $out/first-pass.json and $out/retry.json" >&2
