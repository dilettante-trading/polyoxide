# polyoxide-cli

CLI tool for querying Polymarket APIs.


## Installation

Install using cargo

```bash
cargo install polyoxide-cli
```

Or download binaries directly from Github releases

```
curl -fsSL https://raw.githubusercontent.com/dilettante-trading/polyoxide/main/scripts/install.sh | sh
```

## Usage

```bash
polyoxide <API> <COMMAND> [OPTIONS]
```

### Gamma API

Query market data from the Gamma API.

#### Markets

```bash
# List markets
polyoxide gamma markets list --limit 10 --active true

# Get a market by condition ID
polyoxide gamma markets get <CONDITION_ID>
```

Display all supported features

```
polyoxide gamma --help
```

### Data API

Query Data API.

#### Markets

```bash
# List user activity
polyoxide data positions --user 0x56687bf447db6ffa42ffe2204a05edaa20f55839 activity

# List builders leaderboard
polyoxide data builders leaderboard
```

Display all supported features

```
polyoxide data --help
```

### WebSocket

Subscribe to real-time market data and user updates.

```bash
# Subscribe to market channel (order book, prices)
polyoxide ws market <ASSET_ID>

# Only show trades, exit after 10 messages
polyoxide ws market --filter trade -n 10 <ASSET_ID>

# Run for 30 seconds with summary output
polyoxide ws market -t 30s -f summary <ASSET_ID>

# Subscribe to user channel (requires API credentials)
polyoxide ws user <MARKET_ID>
```

Display all supported features

```
polyoxide ws --help
```

## Shell completions

For convenience, shell completions can be generated too

```
# Generate completions for Fish shell
polyoxide completions fish
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
