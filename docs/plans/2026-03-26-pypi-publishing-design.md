# Publish `polyoxide` to PyPI

**Date:** 2026-03-26
**Status:** Approved

## Scope

1. Add maturin-based wheel build jobs to the existing `release.yml` workflow
2. Export all type classes from `__init__.py`
3. Publish to PyPI alongside crates.io on every version bump

## CI/CD Changes (`release.yml`)

### New job: `build-python`

Runs in parallel with `build-cli`. Uses `maturin build --release -m polyoxide-py/Cargo.toml` via a matrix:

| OS | Target | Wheels |
|----|--------|--------|
| ubuntu-latest | x86_64-unknown-linux-gnu | manylinux x86_64 |
| ubuntu-latest | aarch64-unknown-linux-gnu | manylinux aarch64 — _deferred: aws-lc-sys cross-compilation fails_ |
| macos-latest | x86_64-apple-darwin | macOS x86_64 |
| macos-latest | aarch64-apple-darwin | macOS aarch64 |
| windows-latest | x86_64-pc-windows-msvc | Windows x86_64 |

> **Note:** The aarch64-unknown-linux-gnu entry is commented out in `release.yml` because aws-lc-sys cross-compilation fails, so only 4 wheel targets currently ship.

Each job produces a single abi3 wheel (already configured in pyproject.toml — covers all Python >=3.9). An sdist is built once on linux x86_64.

### New job: `publish-python`

Runs after `build-python`. Uses PyPI trusted publishing or an API token (`PYPI_API_TOKEN` secret in a `pypi` GitHub environment). Uploads all wheels + sdist.

## `__init__.py` Change

Export all ~50 type classes registered on the module (Market, Event, Trade, OrderBook, etc.) so users can do `from polyoxide import Market` and use `isinstance()` checks.

## Out of Scope

- Type stubs (`.pyi`) — tracked as follow-up
- Authenticated CLOB endpoints
- README / long description on PyPI
