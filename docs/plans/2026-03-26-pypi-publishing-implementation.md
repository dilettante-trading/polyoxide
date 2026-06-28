# PyPI Publishing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Publish the `polyoxide` Python package to PyPI as part of the existing release workflow.

**Architecture:** Add maturin wheel-build jobs (4 platform targets — aarch64-unknown-linux-gnu deferred because aws-lc-sys cross-compilation fails) and a publish job to `release.yml`. Export all type classes from `__init__.py`. Uses abi3 stable ABI so one wheel per platform covers all Python >=3.9.

**Tech Stack:** maturin, PyO3 abi3, GitHub Actions, PyPI trusted publishing or API token

---

### Task 1: Export all type classes from `__init__.py`

**Files:**
- Modify: `polyoxide-py/python/polyoxide/__init__.py`

**Step 1: Update `__init__.py` to import and export all type classes**

Replace the contents of `polyoxide-py/python/polyoxide/__init__.py` with:

```python
from ._polyoxide import (
    # Async clients
    Gamma,
    DataApi,
    ClobClient,
    # Sync clients
    GammaSync,
    DataApiSync,
    ClobClientSync,
    # Errors
    PolyoxideError,
    ApiError,
    AuthenticationError,
    ValidationError,
    RateLimitError,
    NetworkError,
    TimeoutError,
    # Gamma types
    Market,
    MarketToken,
    Event,
    SeriesInfo,
    SeriesData,
    Tag,
    SportMetadata,
    Team,
    Comment,
    CommentUser,
    CommentReaction,
    CommentPosition,
    CountResponse,
    Cursor,
    SearchResponse,
    SearchProfile,
    UserResponse,
    UserInfo,
    # CLOB types
    ClobMarket,
    ClobMarketToken,
    ListMarketsResponse,
    OrderBook,
    OrderLevel,
    PriceResponse,
    MidpointResponse,
    PriceHistoryPoint,
    PricesHistoryResponse,
    NegRiskResponse,
    FeeRateResponse,
    TickSizeResponse,
    SpreadResponse,
    LastTradePriceResponse,
    LiveActivityEvent,
    CalculatePriceResponse,
    ServerTimeResponse,
    # Data types
    Position,
    ClosedPosition,
    Trade,
    Activity,
    UserValue,
    OpenInterest,
    UserTraded,
    MarketHolders,
    Holder,
    TraderRanking,
    BuilderRanking,
    BuilderVolume,
    LiveVolume,
    MarketVolume,
    HealthResponse,
)

__all__ = [
    # Async clients
    "Gamma",
    "DataApi",
    "ClobClient",
    # Sync clients
    "GammaSync",
    "DataApiSync",
    "ClobClientSync",
    # Errors
    "PolyoxideError",
    "ApiError",
    "AuthenticationError",
    "ValidationError",
    "RateLimitError",
    "NetworkError",
    "TimeoutError",
    # Gamma types
    "Market",
    "MarketToken",
    "Event",
    "SeriesInfo",
    "SeriesData",
    "Tag",
    "SportMetadata",
    "Team",
    "Comment",
    "CommentUser",
    "CommentReaction",
    "CommentPosition",
    "CountResponse",
    "Cursor",
    "SearchResponse",
    "SearchProfile",
    "UserResponse",
    "UserInfo",
    # CLOB types
    "ClobMarket",
    "ClobMarketToken",
    "ListMarketsResponse",
    "OrderBook",
    "OrderLevel",
    "PriceResponse",
    "MidpointResponse",
    "PriceHistoryPoint",
    "PricesHistoryResponse",
    "NegRiskResponse",
    "FeeRateResponse",
    "TickSizeResponse",
    "SpreadResponse",
    "LastTradePriceResponse",
    "LiveActivityEvent",
    "CalculatePriceResponse",
    "ServerTimeResponse",
    # Data types
    "Position",
    "ClosedPosition",
    "Trade",
    "Activity",
    "UserValue",
    "OpenInterest",
    "UserTraded",
    "MarketHolders",
    "Holder",
    "TraderRanking",
    "BuilderRanking",
    "BuilderVolume",
    "LiveVolume",
    "MarketVolume",
    "HealthResponse",
]
```

**Step 2: Add a test that all exports are importable**

Add to `polyoxide-py/tests/test_live_api.py` at the end (before the helpers section):

```python
class TestExports:
    def test_all_types_importable(self):
        """Every name in __all__ should be importable."""
        import polyoxide
        for name in polyoxide.__all__:
            assert hasattr(polyoxide, name), f"polyoxide.{name} not found"

    def test_market_isinstance(self):
        """Type classes should work with isinstance()."""
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=1)
        assert isinstance(markets[0], polyoxide.Market)
```

**Step 3: Build and run tests locally**

```bash
cd polyoxide-py && uv run maturin develop && uv run pytest tests/test_live_api.py::TestExports -v
```

Expected: both tests PASS.

**Step 4: Commit**

```bash
git add polyoxide-py/python/polyoxide/__init__.py polyoxide-py/tests/test_live_api.py
git commit -m "feat(python): export all type classes from polyoxide package"
```

---

### Task 2: Add Python wheel build job to release workflow

**Files:**
- Modify: `.github/workflows/release.yml`

**Step 1: Add `build-python` job**

Insert the following job after `build-cli` in `.github/workflows/release.yml`. It runs in parallel with `build-cli` (both depend on `version`).

```yaml
  build-python:
    name: Build Python (${{ matrix.target }})
    needs: version
    if: needs.version.outputs.should_release == 'true'
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            manylinux: auto
          # TODO: aarch64-unknown-linux-gnu disabled — aws-lc-sys cross-compilation fails
          # - os: ubuntu-latest
          #   target: aarch64-unknown-linux-gnu
          #   manylinux: auto
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ needs.version.outputs.sha }}

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: PyO3/maturin-action@v1
        with:
          target: ${{ matrix.target }}
          args: --release --out dist -m polyoxide-py/Cargo.toml
          manylinux: ${{ matrix.manylinux || 'auto' }}

      - uses: actions/upload-artifact@v4
        with:
          name: wheels-${{ matrix.target }}
          path: dist/*.whl

  build-python-sdist:
    name: Build Python sdist
    needs: version
    if: needs.version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ needs.version.outputs.sha }}

      - uses: PyO3/maturin-action@v1
        with:
          command: sdist
          args: --out dist -m polyoxide-py/Cargo.toml

      - uses: actions/upload-artifact@v4
        with:
          name: wheels-sdist
          path: dist/*.tar.gz
```

**Step 2: Add `publish-python` job**

Insert after `build-python-sdist`:

```yaml
  publish-python:
    name: Publish to PyPI
    needs: [version, build-python, build-python-sdist]
    if: |
      needs.version.outputs.should_release == 'true' &&
      github.repository == 'dilettante-trading/polyoxide'
    runs-on: ubuntu-latest
    environment: pypi
    permissions:
      id-token: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          pattern: wheels-*
          merge-multiple: true
          path: dist

      - uses: PyO3/maturin-action@v1
        with:
          command: upload
          args: --non-interactive --skip-existing dist/*
```

**Step 3: Update the `release` job's `needs` to include `publish-python`**

Change the `release` job's `needs` from:

```yaml
    needs: [version, publish, build-cli]
```

to:

```yaml
    needs: [version, publish, build-cli, publish-python]
```

This ensures the GitHub Release is only created after both crates.io and PyPI publishing succeed.

**Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add PyPI wheel build and publish to release workflow"
```

---

### Task 3: Configure PyPI trusted publishing on GitHub

**This is a manual step — no code changes.**

**Option A: Trusted Publishing (recommended, no secrets needed)**

1. Go to https://pypi.org and register the `polyoxide` project (first publish will create it, or register manually)
2. On PyPI, go to **Manage project** → **Publishing** → **Add a new pending publisher**
3. Fill in:
   - Owner: `dilettante-trading`
   - Repository: `polyoxide`
   - Workflow: `release.yml`
   - Environment: `pypi`
4. The `publish-python` job already has `permissions: id-token: write` and `environment: pypi` configured for this

**Option B: API Token (fallback)**

If trusted publishing doesn't work (e.g., first-time project registration issues):

1. Generate a PyPI API token at https://pypi.org/manage/account/token/
2. Add it as `PYPI_API_TOKEN` secret in the `pypi` GitHub environment
3. Modify the `publish-python` job's maturin upload step:

```yaml
      - uses: PyO3/maturin-action@v1
        env:
          MATURIN_PYPI_TOKEN: ${{ secrets.PYPI_API_TOKEN }}
        with:
          command: upload
          args: --non-interactive --skip-existing dist/*
```

**Step 1: Configure trusted publishing on PyPI (Option A)**

Follow the instructions above in the PyPI web UI.

**Step 2: Create the `pypi` GitHub environment**

Go to repo **Settings** → **Environments** → **New environment** → name it `pypi`. No protection rules needed (the workflow already gates on `should_release` and repo check).

---

### Task 4: Verify end-to-end

**Step 1: Push branch and verify CI passes**

The `python` CI job in `ci.yml` already runs `uv run pytest tests/ -v` which will exercise the new `TestExports` tests.

```bash
git push origin aidanb/python-analysis
```

**Step 2: Verify workflow YAML is valid**

```bash
gh workflow view release.yml
```

Expected: no syntax errors.

**Step 3: Dry-run the release (optional)**

To test without actually publishing, you can trigger the release workflow manually after merging to main. The `--skip-existing` flag on maturin upload makes it safe to re-run.
