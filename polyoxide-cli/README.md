# polyoxide-cli

CLI tool for Polymarket's APIs and streams, and Binance USDⓈ-M market data. Provides read-only access to market data (Gamma), user and trade data (Data), the CLOB's price history, real-time WebSocket streams, and credentials in the OS keychain.

## Installation

Install from crates.io:

```bash
cargo install polyoxide-cli
```

Or download binaries directly from Github releases:

```
curl -fsSL https://raw.githubusercontent.com/dilettante-trading/polyoxide/main/scripts/install.sh | sh
```

Or build from source:

```bash
cargo build -p polyoxide-cli --release
```

## Usage

```
polyoxide <VENUE> <MODULE> <COMMAND> [OPTIONS]
```

Commands are grouped by venue, then by module:

| Command                           | Description                                                       |
|-----------------------------------|-------------------------------------------------------------------|
| `polymarket gamma`                | Query Gamma API (market data)                                     |
| `polymarket data`                 | Query Data API (user positions, trades, builders)                 |
| `polymarket clob`                 | Download CLOB price history; stream the market and user channels  |
| `polymarket rtds`                 | Stream RTDS reference prices                                      |
| `polymarket sports`               | Stream live match scores                                          |
| `polymarket credentials`          | Manage OS keychain credentials (requires `keychain` feature)      |
| `binance usdm`                    | Stream Binance USDⓈ-M futures market data                         |
| `completions`                     | Generate shell completions                                        |

Every socket streams with the verb `stream`. Commands that print API responses print
JSON to stdout.

v0.39.0's commands (`gamma`, `data`, `clob`, `ws` and `credentials` at the top) are gone;
[`docs/rename-manifest.md`](../docs/rename-manifest.md#commands) maps each to its new
command.

---

### Gamma API

Read-only market data. No authentication required.

#### `polymarket gamma markets`

```bash
# List open markets (defaults: limit=20, offset=0, active=true, status=open)
polyoxide polymarket gamma markets list

# Use a preset filter
polyoxide polymarket gamma markets list --preset trending
polyoxide polymarket gamma markets list --preset top-volume
polyoxide polymarket gamma markets list --preset high-liquidity
polyoxide polymarket gamma markets list --preset new
polyoxide polymarket gamma markets list --preset competitive

# Custom filters
polyoxide polymarket gamma markets list --limit 50 --offset 100 --status closed
polyoxide polymarket gamma markets list --volume-min 10000 --liquidity-min 5000 --sort asc

# Get a single market by condition ID or slug
polyoxide polymarket gamma markets get <CONDITION_ID>
polyoxide polymarket gamma markets get-by-slug <SLUG>
```

#### `polymarket gamma events`

```bash
# List events (defaults: limit=20, offset=0, status=open, order=startDate)
polyoxide polymarket gamma events list
polyoxide polymarket gamma events list --featured --limit 10
polyoxide polymarket gamma events list --status closed --volume-min 1000

# Get event by ID or slug
polyoxide polymarket gamma events get <EVENT_ID>
polyoxide polymarket gamma events get-by-slug <SLUG>
```

#### `polymarket gamma tags`

```bash
polyoxide polymarket gamma tags list
polyoxide polymarket gamma tags list --is-carousel true
polyoxide polymarket gamma tags get <TAG_ID>
polyoxide polymarket gamma tags get-by-slug <SLUG>
polyoxide polymarket gamma tags related <TAG_ID>
polyoxide polymarket gamma tags related-by-slug <SLUG>
```

#### `polymarket gamma series`

```bash
polyoxide polymarket gamma series list
polyoxide polymarket gamma series list --status closed
polyoxide polymarket gamma series get <SERIES_ID>
```

#### `polymarket gamma sports`

```bash
polyoxide polymarket gamma sports list
polyoxide polymarket gamma sports teams --league NBA --limit 50
```

#### `polymarket gamma comments`

```bash
polyoxide polymarket gamma comments list
polyoxide polymarket gamma comments list --parent-entity-type event --parent-entity-id 42
polyoxide polymarket gamma comments list --holders-only true --get-positions true
```

---

### Data API

User positions, trades, and aggregate data, from Data API v2. No authentication
required.

**Output.** A single call prints the response envelope as pretty JSON:
`{"data": ..., "pagination": {..., "next_cursor": ...}}`. Field names are
snake_case (`proxy_wallet`, `condition_id`), as v2 returns them.

**Paging.** v2 pages by cursor only, so `--offset` is gone. Every listing
command takes:

- `--cursor <next_cursor>` to fetch the page after a previous one;
- `--all` to walk every page, writing one JSON row per line (JSONL);
- `--max-pages N`, with `--all`, to stop after N pages.

When a walk stops before the last page, whether at `--max-pages` or on an error,
the cursor to resume from is printed to stderr as `next_cursor: <cursor>`.

```bash
polyoxide polymarket data trades list --user 0xADDRESS --all > trades.jsonl
polyoxide polymarket data trades list --all --max-pages 5 2> resume.txt
polyoxide polymarket data trades list --all --cursor "$(sed -n 's/^next_cursor: //p' resume.txt)"
```

`--condition` takes comma-separated condition IDs; `--market` and `-m` are
aliases for it.

#### `polymarket data health`

```bash
polyoxide polymarket data health
```

#### `polymarket data activity`

```bash
# List user activity (--user required), newest first
polyoxide polymarket data activity --user 0xADDRESS
polyoxide polymarket data activity --user 0xADDRESS --activity-type trade,split,tip
polyoxide polymarket data activity --user 0xADDRESS --side buy --sort-direction asc

# Deposits and withdrawals are hidden unless asked for
polyoxide polymarket data activity --user 0xADDRESS --include-deposits-withdrawals

# Full history (the API's default window starts three years back)
polyoxide polymarket data activity --user 0xADDRESS --start 1 --all
```

#### `polymarket data positions`

```bash
# List open positions (--user required, then a subcommand)
polyoxide polymarket data positions --user 0xADDRESS list
polyoxide polymarket data positions --user 0xADDRESS list --condition <CONDITION_ID> --title "search term"

# Redeemable or closed positions
polyoxide polymarket data positions --user 0xADDRESS list --status redeemable
polyoxide polymarket data positions --user 0xADDRESS list --status closed --sort-by realized-pnl --limit 20

# Total value of positions
polyoxide polymarket data positions --user 0xADDRESS value

# User activity (the same as `polymarket data activity`, scoped to the user)
polyoxide polymarket data positions --user 0xADDRESS activity
```

#### `polymarket data trades`

```bash
# List global trades
polyoxide polymarket data trades list

# List trades for a specific user
polyoxide polymarket data trades list --user 0xADDRESS

# Filter by market, side, or amounts
polyoxide polymarket data trades list --condition <CONDITION_ID> --side buy
polyoxide polymarket data trades list --filter-type cash --filter-amount 100
```

#### `polymarket data traded`

```bash
# A user's profile stats; `trades` is the number of distinct markets traded.
# Prints `null` for a wallet the API does not know.
polyoxide polymarket data traded --user 0xADDRESS
```

#### `polymarket data holders`

```bash
# Top holders per outcome token (--condition required, comma-separated)
polyoxide polymarket data holders --condition <CONDITION_ID>
polyoxide polymarket data holders --condition "id1,id2" --limit 50 --min-balance 10

# Add each holder's entry cost and P&L (one condition only)
polyoxide polymarket data holders --condition <CONDITION_ID> --include-pnl
```

#### `polymarket data builders`

```bash
# Builder leaderboard (time-period: day, week, month, all)
polyoxide polymarket data builders leaderboard
polyoxide polymarket data builders leaderboard --time-period week --limit 10

# Builder volume per bucket (time-period is the bucket width)
polyoxide polymarket data builders volume
polyoxide polymarket data builders volume --time-period month --limit 12
```

#### `polymarket data open-interest`

```bash
polyoxide polymarket data open-interest
polyoxide polymarket data open-interest --condition <CONDITION_ID>
```

#### `polymarket data live-volume`

```bash
polyoxide polymarket data live-volume --event-id 42
polyoxide polymarket data live-volume --event-id 42,43
```

---

### CLOB price history

#### `polymarket clob prices download`

A bulk, resumable, rate-limited downloader for CLOB historical prices
(`GET /prices-history`). It writes one dataset file per market, CSV, JSONL or Parquet,
plus a `manifest.jsonl`. Parquet needs the CLI built with the `parquet` feature.

```bash
# Explicit token IDs, comma-separated or repeated
polyoxide polymarket clob prices download --token-id <TOKEN_ID_1>,<TOKEN_ID_2> --out ./prices

# Token IDs from a file, one per line
polyoxide polymarket clob prices download --input tokens.txt --format jsonl

# Discover markets through Gamma, and print the targets without fetching
polyoxide polymarket clob prices download --discover --closed true --min-volume 100000 --dry-run
```

---

### Streams

Every stream shares its options:

- `--format pretty` (the default) prints one human-readable line per event, and
  `--format json` one compact JSON object per line;
- `-n N` exits after printing N events;
- `--timeout 30s` exits after a duration: `30s`, `5m`, `1h`, `500ms`, or a bare number
  of seconds.

Events go to stdout. Connection notices, filters in effect and frames that could not be
read go to stderr, so `--format json` output stays clean JSONL. A stream ends with an
error when its feed fails in a way it cannot skip, such as a closed connection, and
quietly when the reader of stdout goes away, as with `| head -1`.

#### `polymarket clob stream market`

```bash
# Subscribe to the market channel (order book, prices, trades, tick sizes)
polyoxide polymarket clob stream market <ASSET_ID>

# Several assets, comma-separated or as separate arguments
polyoxide polymarket clob stream market <ASSET_ID_1>,<ASSET_ID_2>

# Filter by event type: book, price, trade, tick, and with --custom-features
# best-bid-ask, new-market and market-resolved
polyoxide polymarket clob stream market <ASSET_ID> --filter trade
polyoxide polymarket clob stream market <ASSET_ID> --filter book,price
polyoxide polymarket clob stream market <ASSET_ID> --custom-features --filter best-bid-ask

# Limit output
polyoxide polymarket clob stream market <ASSET_ID> -n 10          # exit after 10 events
polyoxide polymarket clob stream market <ASSET_ID> --timeout 30s  # exit after 30 seconds

# Each event as compact JSON
polyoxide polymarket clob stream market <ASSET_ID> --format json
```

#### `polymarket clob stream user`

Requires API credentials via flags or environment variables.

```bash
# Subscribe to the user channel (orders, trades)
polyoxide polymarket clob stream user <MARKET_ID>

# Credentials via flags
polyoxide polymarket clob stream user <MARKET_ID> --api-key KEY --api-secret SECRET --api-passphrase PASS

# Credentials from OS keychain (feature `keychain`)
polyoxide polymarket clob stream user <MARKET_ID> --credential-source keychain

# Filter by event type: order, trade
polyoxide polymarket clob stream user <MARKET_ID> --filter order
polyoxide polymarket clob stream user <MARKET_ID> --format json --timeout 5m
```

#### `polymarket rtds stream`

No credentials needed. RTDS reference prices: Binance spot, Chainlink spot, or Chainlink
TWAP (the default). A symbol-filtered subscription first receives a backfill, which is
noted on stderr.

```bash
# Chainlink TWAP over 30 seconds, every symbol
polyoxide polymarket rtds stream

# Some symbols, comma-separated
polyoxide polymarket rtds stream --topic chainlink --symbol btc/usd,eth/usd
polyoxide polymarket rtds stream --topic binance --symbol btcusdt --format json -n 10
polyoxide polymarket rtds stream --topic twap --window 60
```

#### `polymarket sports stream`

No credentials needed. Reconnects on its own; connection notices go to stderr.
A reconnect refused in a way retrying cannot fix, such as a `404`, ends the
command with an error.

```bash
# Every live match in every league
polyoxide polymarket sports stream

# Some leagues, or some games (numeric ids and cricket's id-prefixed ids alike)
polyoxide polymarket sports stream --league atp,wta
polyoxide polymarket sports stream --game 1712005,id2704098174740616

# Drop the server's repeated frames, print JSON, stop after 10 updates
polyoxide polymarket sports stream --changes-only --format json -n 10
```

#### `binance usdm stream`

Binance USDⓈ-M futures market data. No credentials needed. Each `--kind` is streamed for
each `--symbol`; connection notices go to stderr, and `--format json` prints each frame's
`{"stream", "data"}` envelope.

```bash
# Every symbol's mark price and funding, each second
polyoxide binance usdm stream --all-mark-prices

# Trades and hourly klines for two symbols
polyoxide binance usdm stream --symbol BTCUSDT,ETHUSDT --kind agg-trade,kline-1h

# Top 20 book levels every 100 ms, as JSON envelopes, stop after 10 updates
polyoxide binance usdm stream --symbol BTCUSDT --kind depth20 --format json -n 10
```

Kinds: `agg-trade`, `book-ticker`, `depth5`, `depth10`, `depth20`, `kline-<interval>`
(`1m` to `1M`), `mark-price`, `ticker`.

---

### Credentials (feature `keychain`)

Manage API credentials stored in the OS keychain (macOS Keychain, Windows Credential
Manager, Linux Secret Service), as `polyoxide polymarket credentials <store|show|delete>
<kind>`. Polymarket's kinds are `clob` (the `polyoxide-clob` service, which
`Account::from_keychain` reads) and `builder` (the relay's `polyoxide-relay` service,
which `BuilderAccount::from_keychain` reads).

#### `polymarket credentials store`

Writes only the fields given. Every value is checked before any is written: none may be
empty, a private key must parse, and the relayer API key and its address go together.

```bash
# Store CLOB API credentials
polyoxide polymarket credentials store clob --private-key 0x... --api-key KEY --api-secret SECRET --api-passphrase PASS

# Store the relay's builder credentials
polyoxide polymarket credentials store builder --private-key 0x... --api-key KEY --api-secret SECRET --passphrase PASS

# Store a relayer API key and its address
polyoxide polymarket credentials store builder --private-key 0x... --relayer-api-key KEY --relayer-api-key-address 0x...
```

#### `polymarket credentials show`

Prints each field as `present`, `not found` or `error: …`. A secret is never printed; the
relayer API key's address, which is not secret, is printed when present.

```bash
polyoxide polymarket credentials show clob
polyoxide polymarket credentials show builder
```

#### `polymarket credentials delete`

```bash
# Delete every field of a kind from the keychain
polyoxide polymarket credentials delete clob
polyoxide polymarket credentials delete builder
```

---

### Shell completions

```bash
polyoxide completions bash
polyoxide completions zsh
polyoxide completions fish
polyoxide completions powershell
polyoxide completions elvish
```

---

## Environment Variables

The CLOB's user stream reads credentials from environment variables when flags are not provided:

| Variable                     | Used by                            | Description        |
|------------------------------|------------------------------------|--------------------|
| `POLYMARKET_API_KEY`         | `polymarket clob stream user`      | L2 API key         |
| `POLYMARKET_API_SECRET`      | `polymarket clob stream user`      | L2 API secret      |
| `POLYMARKET_API_PASSPHRASE`  | `polymarket clob stream user`      | L2 API passphrase  |

The `polymarket gamma` and `polymarket data` commands are read-only and do not require authentication.

`RUST_LOG` sets how much the libraries log to stderr. The default is `warn`,
which shows what they recover from on their own, such as a reconnect being
retried or a rate limit being waited out. `RUST_LOG=info` or `RUST_LOG=debug`
shows more.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
