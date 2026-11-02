#!/usr/bin/env bash
# The per-push browser smoke suite (spec D13). It starts the local smoke server, exports the
# addresses the wasm32 tests read at compile time (`env!`), and runs tests/browser_smoke.rs in
# headless Chrome, inside a dedicated Worker, through wasm-bindgen-test-runner. The server stops
# when this script does.
#
# The runner reads Chrome's arguments from polyoxide-wasm-smoke/webdriver.json, in the working
# directory cargo gives the test (the package root). It starts Chrome with `--no-sandbox` there:
# on a runner whose kernel restricts unprivileged user namespaces, and in a container, Chrome
# cannot create its sandbox and exits before the driver connects.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ ! -f polyoxide-wasm-smoke/webdriver.json ]; then
  echo "wasm_smoke: polyoxide-wasm-smoke/webdriver.json is missing; the runner would start Chrome with its own arguments" >&2
  exit 1
fi

for tool in wasm-bindgen wasm-bindgen-test-runner; do
  if ! command -v "$tool" >/dev/null; then
    echo "wasm_smoke: $tool is not on PATH; run inside \`nix develop\` or install wasm-bindgen-cli" >&2
    exit 1
  fi
done

# The runner and the wasm-bindgen crate must be the same version, or the runner refuses the
# module.
locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [ "$locked" != "$installed" ]; then
  echo "wasm_smoke: wasm-bindgen-cli is $installed but Cargo.lock pins wasm-bindgen $locked; they must match" >&2
  exit 1
fi

# The runner takes its driver from CHROMEDRIVER. A GitHub runner has it beside CHROMEWEBDRIVER.
if [ -z "${CHROMEDRIVER:-}" ]; then
  if command -v chromedriver >/dev/null; then
    CHROMEDRIVER=$(command -v chromedriver)
  elif [ -x "${CHROMEWEBDRIVER:-}/chromedriver" ]; then
    CHROMEDRIVER="$CHROMEWEBDRIVER/chromedriver"
  else
    echo "wasm_smoke: no chromedriver on PATH, in CHROMEDRIVER or in CHROMEWEBDRIVER" >&2
    exit 1
  fi
  export CHROMEDRIVER
fi

work=$(mktemp -d)
server_pid=""
cleanup() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

# The server is a host build, so it runs without the wasm32 flags set below.
cargo build --quiet -p polyoxide-wasm-smoke --bin wasm-smoke-server
target_dir=$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
"$target_dir/debug/wasm-smoke-server" "$work/smoke.env" &
server_pid=$!

for _ in $(seq 1 100); do
  [ -s "$work/smoke.env" ] && break
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "wasm_smoke: the smoke server exited before it wrote its addresses" >&2
    exit 1
  fi
  sleep 0.1
done
if [ ! -s "$work/smoke.env" ]; then
  echo "wasm_smoke: the smoke server wrote no addresses within ten seconds" >&2
  exit 1
fi

# POLYOXIDE_SMOKE_HTTP, POLYOXIDE_SMOKE_WS and POLYOXIDE_SMOKE_SLOW, for the tests' `env!`.
set -a
. "$work/smoke.env"
set +a

RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
WASM_BINDGEN_TEST_TIMEOUT=120 \
  cargo test -p polyoxide-wasm-smoke --target wasm32-unknown-unknown --test browser_smoke
