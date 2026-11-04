---
paths:
  - "polyoxide-py/**"
---

# Python bindings (`polyoxide-py`)

PyO3 bindings for Polymarket's clob, gamma and data modules (`publish = false`; the wheels are
built and published to PyPI by a separate step of the release workflow).

- The Data API is `DataApi()` / `DataApiSync()`, each route a method (`DataApi().trades()`),
  with the row classes, `Page` and the page iterators on `polyoxide`. Polymarket retired Data
  API v1 on 2026-11-02, so the routes are v2's and `polyoxide.v2` is gone. Enum arguments take
  the exact wire spelling.
- **Errors.** A Data API error with its structured body maps by `code`; every other error maps by its class, one class to one
  exception (`exception_for` in `src/error.rs`), never by its message (C4): `Network`
  `NetworkError`, `Unavailable` `UnavailableError`, `RateLimited` `RateLimitError`, `Unauthorized`
  `AuthenticationError`, `InvalidRequest` `ValidationError`, `VenueRefusal` `ApiError`,
  `Restricted` `RestrictedError`, `Decode` `DecodeError`. `TimeoutError`, which only the Data
  API's `request_timeout` raises, subclasses `UnavailableError`, and `tests/test_errors_offline.py`
  asserts each class's exact type. Every SDK exception carries `status`, `code`, `retryable`,
  `trace_id`, `parameter` and `retry_after`, which are `None` unless the error came with the
  Data API's structured body.
- Three tests hold the bindings up:
  - `every_v2_getter_reads_its_own_key` in `src/types/data.rs`, because `get_field` returns
    `None` for a missing key, which stub consistency cannot see;
  - `test_stub_consistency.py` checks `__init__.pyi`'s members, and the Data client's method
    signatures, against the compiled module;
  - `test_data_offline.py` calls every route with every argument against a local server.
- Run the suite with `cd polyoxide-py && uv sync --reinstall-package polyoxide && uv run pytest tests/`.
  Plain `uv sync` keeps a stale in-tree extension, so a local `ImportError` or old behaviour
  usually means the extension needs `--reinstall-package polyoxide`.
