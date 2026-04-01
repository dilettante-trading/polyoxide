# Polyoxide

Python SDK for [Polymarket](https://polymarket.com) APIs, powered by Rust via [PyO3](https://pyo3.rs).

Provides async and sync clients for the three Polymarket API surfaces:

| Client | API | Description |
|--------|-----|-------------|
| `Gamma` / `GammaSync` | Gamma | Market data, events, series, tags, search |
| `ClobClient` / `ClobClientSync` | CLOB | Order book, prices, spreads, trade history |
| `DataApi` / `DataApiSync` | Data | User positions, trades, leaderboard, volume |

## Installation

```bash
pip install polyoxide
```

Wheels are published for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64).

## Quick Start

### Async

```python
import asyncio
from polyoxide import Gamma

async def main():
    gamma = Gamma()
    markets = await gamma.markets().list(limit=5, open=True)
    for m in markets:
        print(f"{m.question}  —  {m.slug}")

asyncio.run(main())
```

### Sync

```python
from polyoxide import GammaSync

gamma = GammaSync()
markets = gamma.markets().list(limit=5, open=True)
for m in markets:
    print(f"{m.question}  —  {m.slug}")
```

## Rust Core

The underlying HTTP and serialization layer is written in Rust (the [polyoxide](https://crates.io/crates/polyoxide) workspace), giving native performance with a Pythonic API.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
