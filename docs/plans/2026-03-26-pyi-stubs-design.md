# `.pyi` Type Stubs for `polyoxide`

**Date:** 2026-03-26
**Status:** Approved

## Scope

Hand-written `.pyi` stub file covering all public types, clients, namespaces, and errors with full type annotations and one-liner docstrings. Plus a `py.typed` PEP 561 marker.

## Files

- `polyoxide-py/python/polyoxide/__init__.pyi` — all type stubs
- `polyoxide-py/python/polyoxide/py.typed` — empty PEP 561 marker

## Approach

- Single `__init__.pyi` since users only import from `polyoxide`
- Properties on type wrappers return `Any` (dynamic JSON values), with precise types where Rust source type is obvious
- Async namespace methods return `Coroutine[Any, Any, T]`; sync return `T` directly
- One-liner docstrings per method
- Covers all 50 type classes, 6 client classes, ~36 namespace classes, 7 error classes, ~75 methods

## Out of Scope

- Runtime type checking
- Auto-generation from Rust source
