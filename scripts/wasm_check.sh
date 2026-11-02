#!/usr/bin/env bash
# Check and lint the crates that support wasm32-unknown-unknown (spec D1), crate
# by crate with their features named. The first failure stops it.
#
# `cargo check` and `cargo clippy` both run, since clippy -D warnings is the
# gate CI holds the native build to. The browser's getrandom backend is chosen
# by a cfg flag, not a feature, so this script sets RUSTFLAGS itself, replacing
# any inherited value (a host `target-cpu`, say, would override a config
# file's and break the target); CARGO_ENCODED_RUSTFLAGS outranks RUSTFLAGS, so
# it goes too. With `--target`, cargo does not pass RUSTFLAGS to build scripts,
# so the host build cache is untouched.
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
