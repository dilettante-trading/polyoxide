# polyoxide-core

Core utilities and shared types for Polyoxide Polymarket API clients.

This crate provides common functionality used by both `polyoxide-clob` and `polyoxide-gamma`:

- HTTP client configuration and building
- Shared error types
- Request building utilities
- Query parameter handling

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-core/).

## Usage

This crate is typically used as a dependency by other Polyoxide crates and not directly by end users. If you want to interact with Polymarket APIs, use the main [`polyoxide`](https://crates.io/crates/polyoxide) crate instead.

## Features

- **Client Building**: Configurable HTTP client with timeout and connection pooling
- **Error Handling**: Unified error types for API operations
- **Request Utilities**: Builder pattern for constructing API requests

## Installation

```toml
[dependencies]
polyoxide-core = "0.1.0"
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
