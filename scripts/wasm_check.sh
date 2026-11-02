#!/usr/bin/env bash
# Check and lint the crates that support wasm32-unknown-unknown (spec D1), crate
# by crate with their features named. The first failure stops it.
#
# `cargo check` and `cargo clippy` both run, since clippy -D warnings is the
# gate CI holds the native build to. With getrandom 0.3.4 locked, its `wasm_js`
# feature alone selects the browser backend; the `--cfg getrandom_backend` flag
# is kept (spec D1) for consumers that resolve an older 0.3.x, so this script
# sets RUSTFLAGS itself, replacing any inherited value (a host `target-cpu`,
# say, would override a config file's and break the target);
# CARGO_ENCODED_RUSTFLAGS outranks RUSTFLAGS, so it goes too. With `--target`,
# cargo does not pass RUSTFLAGS to build scripts, so the host build cache is
# untouched.
#
# Native-only and out of the set: polyoxide-kalshi, the relay, rtds, the
# polyoxide umbrella, polyoxide-cli, polyoxide-py and every `keychain` feature.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

unset CARGO_ENCODED_RUSTFLAGS
export RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

target=wasm32-unknown-unknown

check() {
    local package=$1
    shift
    echo "==> ${package} $*"
    cargo check --package "${package}" --target "${target}" "$@"
    cargo clippy --package "${package}" --target "${target}" "$@" -- -D warnings
}

check polyoxide-rt
check polyoxide-venue --features market,trading
check polyoxide-core
check polyoxide-ws
check polyoxide-binance --no-default-features --features usdm,usdm-ws
check polyoxide-polymarket --no-default-features \
    --features clob,gamma,data,perps,perps-ws,sports,clob-ws

# The smoke suite's browser tests (tests/browser_smoke.rs) are compiled out on the host and
# built by no step above, so lint them here. They read the smoke server's addresses with `env!`,
# which needs a value to compile; these placeholders are for this lint only, and
# scripts/wasm_smoke.sh, which runs the tests, takes the real addresses from the server.
echo "==> polyoxide-wasm-smoke --tests"
POLYOXIDE_SMOKE_HTTP=http://127.0.0.1:0 \
POLYOXIDE_SMOKE_WS=ws://127.0.0.1:0 \
POLYOXIDE_SMOKE_SLOW=http://127.0.0.1:0 \
POLYOXIDE_SMOKE_STALL=http://127.0.0.1:0 \
    cargo clippy --package polyoxide-wasm-smoke --tests --target "${target}" -- -D warnings

# tests/browser_live.rs is behind the `live` feature (it calls real hosts, and scripts/wasm_live.sh
# runs it nightly), so the lint above skips it. Lint it here as well: this compiles it and runs
# nothing, so a change to core, rt, ws or test-support that breaks it fails this push and not the
# next night's job.
echo "==> polyoxide-wasm-smoke --tests --features live"
POLYOXIDE_SMOKE_HTTP=http://127.0.0.1:0 \
POLYOXIDE_SMOKE_WS=ws://127.0.0.1:0 \
POLYOXIDE_SMOKE_SLOW=http://127.0.0.1:0 \
POLYOXIDE_SMOKE_STALL=http://127.0.0.1:0 \
    cargo clippy --package polyoxide-wasm-smoke --tests --features live --target "${target}" -- -D warnings
