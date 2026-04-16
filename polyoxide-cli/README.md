# polyoxide-cli

CLI tool for querying Polymarket APIs. Provides read-only access to market data (Gamma), user/trade data (Data), and real-time WebSocket streams.

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
polyoxide <COMMAND> [OPTIONS]
```

Top-level commands:

| Command       | Description                                       |
|---------------|---------------------------------------------------|
| `gamma`       | Query Gamma API (market data)                     |
| `data`        | Query Data API (user positions, trades, builders) |
| `ws`          | Subscribe to WebSocket channels (real-time)       |
| `credentials` | Manage OS keychain credentials (requires `keychain` feature) |
| `completions` | Generate shell completions                        |

All output is JSON printed to stdout.

---

### Gamma API

Read-only market data. No authentication required.

#### `gamma markets`

```bash
# List open markets (defaults: limit=20, offset=0, active=true, status=open)
polyoxide gamma markets list

# Use a preset filter
polyoxide gamma markets list --preset trending
polyoxide gamma markets list --preset top-volume
polyoxide gamma markets list --preset high-liquidity
polyoxide gamma markets list --preset new
polyoxide gamma markets list --preset competitive

# Custom filters
polyoxide gamma markets list --limit 50 --offset 100 --status closed
polyoxide gamma markets list --volume-min 10000 --liquidity-min 5000 --sort asc

# Get a single market by condition ID or slug
polyoxide gamma markets get <CONDITION_ID>
polyoxide gamma markets get-by-slug <SLUG>
```

#### `gamma events`

```bash
# List events (defaults: limit=20, offset=0, status=open, order=startDate)
polyoxide gamma events list
polyoxide gamma events list --featured --limit 10
polyoxide gamma events list --status closed --volume-min 1000

# Get event by ID, slug, or find related events
polyoxide gamma events get <EVENT_ID>
polyoxide gamma events get-by-slug <SLUG>
polyoxide gamma events related <SLUG>
```

#### `gamma tags`

```bash
polyoxide gamma tags list
polyoxide gamma tags list --is-carousel true
polyoxide gamma tags get <TAG_ID>
polyoxide gamma tags get-by-slug <SLUG>
polyoxide gamma tags related <TAG_ID>
polyoxide gamma tags related-by-slug <SLUG>
```

#### `gamma series`

```bash
polyoxide gamma series list
polyoxide gamma series list --status closed
polyoxide gamma series get <SERIES_ID>
```

#### `gamma sports`

```bash
polyoxide gamma sports list
polyoxide gamma sports teams --league NBA --limit 50
```

#### `gamma comments`

```bash
polyoxide gamma comments list
polyoxide gamma comments list --parent-entity-type event --parent-entity-id 42
polyoxide gamma comments list --holders-only true --get-positions true
```

---

### Data API

User positions, trades, and aggregate data. No authentication required.

#### `data health`

```bash
polyoxide data health
```

#### `data activity`

```bash
# List user activity (--user required)
polyoxide data activity --user 0xADDRESS
polyoxide data activity --user 0xADDRESS --activity-type trade,split
polyoxide data activity --user 0xADDRESS --side buy --sort-by tokens --sort-direction asc
```

#### `data positions`

```bash
# List open positions (--user required, then a subcommand)
polyoxide data positions --user 0xADDRESS list
polyoxide data positions --user 0xADDRESS list --redeemable --sort-by cash-pnl
polyoxide data positions --user 0xADDRESS list --market <CONDITION_ID> --title "search term"

# Total value of positions
polyoxide data positions --user 0xADDRESS value

# Closed positions
polyoxide data positions --user 0xADDRESS closed
polyoxide data positions --user 0xADDRESS closed --sort-by realized-pnl --limit 20

# User activity (same as top-level activity, scoped to user)
polyoxide data positions --user 0xADDRESS activity
```

#### `data trades`

```bash
# List global trades
polyoxide data trades list

# List trades for a specific user
polyoxide data trades list --user 0xADDRESS

# Filter by market, side, or amounts
polyoxide data trades list --market <CONDITION_ID> --side buy
polyoxide data trades list --filter-type cash --filter-amount 100
```

#### `data traded`

```bash
# Get markets traded by a user
polyoxide data traded --user 0xADDRESS
```

#### `data holders`

```bash
# Top holders for markets (comma-separated condition IDs)
polyoxide data holders --market <CONDITION_ID>
polyoxide data holders --market "id1,id2" --limit 50 --min-balance 10
```

#### `data builders`

```bash
# Builder leaderboard (time-period: day, week, month, all)
polyoxide data builders leaderboard
polyoxide data builders leaderboard --time-period week --limit 10

# Builder volume time series
polyoxide data builders volume
polyoxide data builders volume --time-period month
```

#### `data open-interest`

```bash
polyoxide data open-interest
polyoxide data open-interest --market <CONDITION_ID>
```

#### `data live-volume`

```bash
polyoxide data live-volume --event-id 42
```

---

### WebSocket

Subscribe to real-time market data and user updates.

#### `ws market`

```bash
# Subscribe to market channel (order book, prices, trades, tick sizes)
polyoxide ws market <ASSET_ID>

# Multiple assets
polyoxide ws market <ASSET_ID_1> <ASSET_ID_2>

# Filter by event type: book, price, trade, tick
polyoxide ws market <ASSET_ID> --filter trade
polyoxide ws market <ASSET_ID> --filter book --filter price

# Limit output
polyoxide ws market <ASSET_ID> -n 10                   # exit after 10 messages
polyoxide ws market <ASSET_ID> --timeout 30s            # exit after 30 seconds

# Output format: pretty (default), json, summary
polyoxide ws market <ASSET_ID> --format summary
polyoxide ws market <ASSET_ID> --format json
```

Duration syntax for `--timeout`: `30s`, `5m`, `1h`, `500ms`, or bare number (seconds).

#### `ws user`

Requires API credentials via flags or environment variables.

```bash
# Subscribe to user channel (orders, trades)
polyoxide ws user <MARKET_ID>

# Credentials via flags
polyoxide ws user <MARKET_ID> --api-key KEY --api-secret SECRET --api-passphrase PASS

# Credentials from OS keychain (feature `keychain`)
polyoxide ws user <MARKET_ID> --credential-source keychain

# Filter by event type: order, trade
polyoxide ws user <MARKET_ID> --filter order

# Same --format, -n, --timeout options as ws market
polyoxide ws user <MARKET_ID> --format summary --timeout 5m
```

---

### Credentials (feature `keychain`)

Manage API credentials stored in the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service).

#### `credentials store`

```bash
# Store CLOB API credentials
polyoxide credentials store clob --private-key 0x... --api-key KEY --api-secret SECRET --api-passphrase PASS

# Store Relay builder credentials
polyoxide credentials store relay --private-key 0x... --api-key KEY --api-secret SECRET --passphrase PASS

# Store Relay relayer API key credentials
polyoxide credentials store relay --private-key 0x... --relayer-api-key KEY --relayer-api-key-address 0x...
```

#### `credentials show`

```bash
# Check which CLOB credentials are present in the keychain
polyoxide credentials show clob

# Check which Relay credentials are present
polyoxide credentials show relay
```

#### `credentials delete`

```bash
# Delete all CLOB credentials from the keychain
polyoxide credentials delete clob

# Delete all Relay credentials from the keychain
polyoxide credentials delete relay
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

The WebSocket user channel reads credentials from environment variables when flags are not provided:

| Variable                     | Used by    | Description        |
|------------------------------|------------|--------------------|
| `POLYMARKET_API_KEY`         | `ws user`  | L2 API key         |
| `POLYMARKET_API_SECRET`      | `ws user`  | L2 API secret      |
| `POLYMARKET_API_PASSPHRASE`  | `ws user`  | L2 API passphrase  |

The `gamma` and `data` commands are read-only and do not require authentication.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
