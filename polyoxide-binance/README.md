# polyoxide-binance

Rust client library for Binance's APIs, one feature per module: `usdm`. Part of the
[polyoxide](https://crates.io/crates/polyoxide) workspace, whose umbrella crate, `polyoxide`,
re-exports it as `polyoxide::binance` under its `binance`, `binance-usdm` and `binance-usdm-ws`
features.

Each module has its own README:

| Module | Feature | API |
|---|---|---|
| [`usdm`](src/usdm/README.md) | `usdm`; its market streams, `usdm::ws`, under `usdm-ws` | USDⓈ-M futures public market data on `fapi.binance.com`, and its market streams on `fstream.binance.com`; read-only, no credentials |

`usdm` is on by default, and `usdm-ws` turns it on. `test-server` exposes the captured stream
frames, `usdm::ws::fixtures`, to integration tests with `usdm-ws`; it is not for consumers.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-binance/).

## Installation

```toml
[dependencies]
polyoxide-binance = "0.41"
```
