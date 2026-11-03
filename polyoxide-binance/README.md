# polyoxide-binance

Rust client library for Binance's APIs, one feature per module: `usdm` and `spot`. Part of the
[polyoxide](https://crates.io/crates/polyoxide) workspace, whose umbrella crate, `polyoxide`,
re-exports it as `polyoxide::binance` under its `binance`, `binance-usdm`, `binance-usdm-ws`,
`binance-spot` and `binance-spot-ws` features.

Each module has its own README:

| Module | Feature | API |
|---|---|---|
| [`usdm`](src/usdm/README.md) | `usdm`; its market streams, `usdm::ws`, under `usdm-ws` | USDⓈ-M futures public market data on `fapi.binance.com`, and its market streams on `fstream.binance.com`; read-only, no credentials |
| [`spot`](src/spot/README.md) | `spot`; its market streams, `spot::ws`, under `spot-ws` | Spot public market data on `api.binance.com`, and its market streams on `stream.binance.com`; read-only, no credentials |

`usdm` and `spot` are on by default, and `usdm-ws` turns `usdm` on as `spot-ws` turns `spot` on.
`test-server` exposes the captured stream frames, `usdm::ws::fixtures` with `usdm-ws` and
`spot::ws::fixtures` with `spot-ws`, to integration tests; it is not for consumers.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-binance/).

## Installation

```toml
[dependencies]
polyoxide-binance = "0.45"
```
