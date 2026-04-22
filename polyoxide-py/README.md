# Polyoxide

Python SDK for [Polymarket](https://polymarket.com) APIs, powered by Rust via [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs/).

Every client comes in async and sync variants. Async methods return Python awaitables; sync methods block via an internal Tokio runtime and release the GIL while waiting.

| Async | Sync | API | Description |
|-------|------|-----|-------------|
| `Gamma` | `GammaSync` | Gamma | Market data, events, series, tags, comments, sports, search, users |
| `ClobClient` | `ClobClientSync` | CLOB | Order book, prices, spreads, trade history, fee rates |
| `DataApi` | `DataApiSync` | Data | User positions/trades/activity, leaderboard, holders, volume, open interest |

## Installation

```bash
pip install polyoxide
```

Wheels are published for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64).

## Quick Start

### Async (Gamma -- market data)

```python
import asyncio
from polyoxide import Gamma

async def main():
    gamma = Gamma()
    markets = await gamma.markets().list(limit=5, open=True)
    for m in markets:
        print(f"{m.question}  --  {m.slug}")

asyncio.run(main())
```

### Sync (Gamma)

```python
from polyoxide import GammaSync

gamma = GammaSync()
markets = gamma.markets().list(limit=5, open=True)
for m in markets:
    print(f"{m.question}  --  {m.slug}")
```

### CLOB (order book data)

```python
from polyoxide import ClobClientSync

clob = ClobClientSync()
book = clob.markets().order_book("TOKEN_ID")
print(book.bids, book.asks)

spread = clob.markets().spread("TOKEN_ID")
print(spread)
```

### Data API (user positions and leaderboard)

```python
import asyncio
from polyoxide import DataApi

async def main():
    data = DataApi()

    # Leaderboard
    leaders = await data.leaderboard().get(limit=10, time_period="WEEK")
    for t in leaders:
        print(t.username, t.pnl)

    # User positions
    positions = await data.user("0xADDRESS").list_positions(limit=5)
    for p in positions:
        print(p.title, p.size)

asyncio.run(main())
```

## Client API Reference

### Gamma / GammaSync

Constructed with optional `base_url`, `timeout_ms`, and `pool_size` keyword arguments.

| Namespace | Methods |
|-----------|---------|
| `.markets()` | `get(id)`, `get_by_slug(slug)`, `list(...)`, `tags(id)` |
| `.events()` | `get(id)`, `get_by_slug(slug)`, `list(...)`, `tags(id)`, `tweet_count(id)`, `comment_count(id)` |
| `.series()` | `get(id)`, `list(...)` |
| `.tags()` | `get(id)`, `get_by_slug(slug)`, `list(...)`, `get_related(id)`, `get_related_by_slug(slug)` |
| `.comments()` | `get(id)`, `list(...)`, `by_user(address)` |
| `.sports()` | `list()`, `market_types()`, `list_teams(...)` |
| `.search()` | `public_search(query, ...)` |
| `.user()` | `get(address)` |
| `.health()` | `ping()` |

### ClobClient / ClobClientSync

No arguments required (uses public/unauthenticated endpoints).

| Namespace | Methods |
|-----------|---------|
| `.markets()` | `get(condition_id)`, `get_by_token_ids(token_ids)`, `list()`, `order_book(token_id)`, `price(token_id, side)`, `midpoint(token_id)`, `prices_history(token_id)`, `neg_risk(token_id)`, `fee_rate(token_id)`, `tick_size(token_id)`, `spread(token_id)`, `last_trade_price(token_id)`, `live_activity(condition_id)`, `simplified()`, `sampling()`, `sampling_simplified()`, `calculate_price(token_id, side, amount)` |
| `.health()` | `ping()`, `server_time()` |

### DataApi / DataApiSync

Constructed with optional `base_url`, `timeout_ms`, and `pool_size` keyword arguments.

| Namespace | Methods |
|-----------|---------|
| `.user(address)` | `list_positions(...)`, `positions_value(...)`, `closed_positions(...)`, `trades(...)`, `activity(...)`, `traded()` |
| `.trades()` | `list(...)` |
| `.holders()` | `list(markets, ...)` |
| `.open_interest()` | `get(...)` |
| `.live_volume()` | `get(event_id)` |
| `.leaderboard()` | `get(...)` |
| `.builders()` | `leaderboard(...)`, `volume(...)` |
| `.health()` | `ping()` |

## Result Objects

All response objects expose named properties matching the upstream JSON fields, plus:

- `.to_dict()` -- convert to a plain Python `dict`
- `str(obj)` -- JSON string representation
- `repr(obj)` -- type-annotated representation

## Error Handling

All exceptions inherit from `PolyoxideError`:

```python
from polyoxide import PolyoxideError, ApiError, RateLimitError

try:
    markets = gamma.markets().list()
except RateLimitError:
    print("slow down")
except ApiError as e:
    print(f"API error: {e}")
except PolyoxideError as e:
    print(f"something else: {e}")
```

| Exception | When |
|-----------|------|
| `ApiError` | API returned an error response |
| `AuthenticationError` | Invalid or missing credentials |
| `ValidationError` | Request parameters failed validation |
| `RateLimitError` | Rate limit exceeded (HTTP 429) |
| `NetworkError` | Connection failure |
| `TimeoutError` | Request timed out |

## Async Support

Async clients (`Gamma`, `ClobClient`, `DataApi`) use [pyo3-async-runtimes](https://github.com/PyO3/pyo3-async-runtimes) to bridge Rust futures into Python awaitables. They work with `asyncio.run()`, `await`, and any asyncio-compatible event loop.

Sync clients (`GammaSync`, `ClobClientSync`, `DataApiSync`) execute on a shared background Tokio runtime and release the GIL while blocking, so they are safe to use from threaded Python code.

## Type Stubs

A `.pyi` stub file is included at `python/polyoxide/__init__.pyi` for editor autocomplete and type checking.

## Building from Source

Requires Rust and Python 3.8+.

```bash
pip install maturin
maturin develop --release
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
