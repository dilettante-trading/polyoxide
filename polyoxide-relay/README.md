# polyoxide-relay

Rust client library for Polymarket Relayer API.

The Relay API enables submitting gasless on-chain transactions through Polymarket's relayer service on Polygon. The relayer pays gas fees on behalf of users, supporting two wallet types:

- **Safe wallets** -- Gnosis Safe multisig contracts (must be deployed before first use)
- **Proxy wallets** -- lightweight proxy contracts that auto-deploy on first transaction

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-relay/).

## Installation

```toml
[dependencies]
polyoxide-relay = "0.13"
```

Or use the unified client:

```toml
[dependencies]
polyoxide = { version = "0.13", features = ["full"] }
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `keychain` | No | Enables OS keychain storage for credentials via `keyring` (macOS Keychain, Windows Credential Manager, Linux Secret Service) |

## Authentication

Relay operations require a private key for EIP-712 transaction signing **and** one of two authentication schemes for relay submission:

### Builder API Credentials (HMAC-SHA256)

```rust
use polyoxide_relay::{RelayClient, BuilderAccount, BuilderConfig};

let config = BuilderConfig::new(
    "your-api-key".into(),
    "your-secret".into(),
    Some("your-passphrase".into()),
);
let account = BuilderAccount::new("0xprivatekey...", Some(config))?;
let client = RelayClient::from_account(account)?;
```

### Relayer API Key (static headers)

```rust
use polyoxide_relay::{RelayClient, BuilderAccount};

let account = BuilderAccount::with_relayer_api_key(
    "0xprivatekey...",
    "your-relayer-api-key".into(),
    "0xyour-address".into(),
)?;
let client = RelayClient::from_account(account)?;
```

### OS Keychain (feature `keychain`)

```rust
use polyoxide_relay::{RelayClient, BuilderAccount};

// Load builder credentials from the OS keychain
let account = BuilderAccount::from_keychain()?;
let client = RelayClient::from_account(account)?;

// Or load relayer API key credentials from the OS keychain
let account = BuilderAccount::from_keychain_relayer_api_key()?;
let client = RelayClient::from_account(account)?;
```

## Usage

### Builder Pattern

```rust
use polyoxide_relay::{RelayClient, BuilderAccount, BuilderConfig, WalletType};

let config = BuilderConfig::new("key".into(), "secret".into(), None);
let account = BuilderAccount::new("0xprivatekey...", Some(config))?;

let client = RelayClient::builder()?
    .with_account(account)
    .wallet_type(WalletType::Safe)
    .chain_id(137)               // Polygon mainnet (default)
    .max_concurrent(2)
    .build()?;
```

Or pull settings from environment variables (`RELAYER_URL`, `CHAIN_ID`):

```rust
let client = RelayClient::default_builder()?
    .with_account(account)
    .build()?;
```

### Gasless Redemption

```rust
use alloy::primitives::U256;

let condition_id = [0u8; 32]; // your condition ID
let index_sets = vec![U256::from(1)];

let response = client
    .submit_gasless_redemption(condition_id, index_sets)
    .await?;

println!("Transaction ID: {}", response.transaction_id);
```

### Gasless Redemption with Gas Estimation

```rust
let response = client
    .submit_gasless_redemption_with_gas_estimation(
        condition_id,
        index_sets,
        true, // estimate gas via RPC simulation
    )
    .await?;
```

### Execute Arbitrary Transactions

```rust
use polyoxide_relay::SafeTransaction;
use alloy::primitives::{Address, U256, Bytes};

let tx = SafeTransaction {
    to: "0x...".parse().unwrap(),
    value: U256::ZERO,
    data: Bytes::from(calldata),
    operation: 0, // CALL
};

let response = client.execute(vec![tx], None).await?;
```

### Query Operations (no auth required)

```rust
use alloy::primitives::Address;

// Check Safe deployment status
let deployed = client.get_deployed(safe_address).await?;

// Fetch current nonce
let nonce = client.get_nonce(address).await?;

// Query transaction status
let status = client.get_transaction("tx-id").await?;
println!("State: {}", status.state);

// Measure API latency
let latency = client.ping().await?;
println!("Relay API latency: {}ms", latency.as_millis());
```

### Wallet Address Derivation

```rust
// Derive expected Safe address via CREATE2
let safe_address = client.get_expected_safe()?;

// Derive expected Proxy wallet address via CREATE2
let proxy_address = client.get_expected_proxy_wallet()?;
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `POLYMARKET_PRIVATE_KEY` | Hex-encoded private key for EIP-712 signing |
| `BUILDER_API_KEY` | Builder API key (HMAC auth) |
| `BUILDER_SECRET` | Builder API secret (HMAC auth) |
| `BUILDER_PASS_PHRASE` | Builder API passphrase (HMAC auth, optional) |
| `RELAYER_API_KEY` | Relayer API key (static auth) |
| `RELAYER_API_KEY_ADDRESS` | Address associated with relayer API key |
| `RELAYER_URL` | Custom relayer URL (default: `https://relayer-v2.polymarket.com`) |
| `CHAIN_ID` | Target chain ID (default: `137` for Polygon mainnet) |

## API Coverage

- **Transaction Submission**: Sign and submit gasless transactions via Safe or Proxy wallets
- **Gasless Redemptions**: Redeem CTF positions without holding MATIC
- **Gas Estimation**: Simulate redemptions against Polygon RPC for accurate gas limits
- **Nonce Management**: Fetch current nonce from the relayer
- **Deployment Check**: Verify whether a Safe wallet is deployed on-chain
- **Transaction Status**: Query the state of submitted relay transactions
- **Wallet Derivation**: Compute expected Safe and Proxy wallet addresses via CREATE2
- **Multi-Send Batching**: Automatically batch multiple transactions via Gnosis Safe MultiSend

## Supported Chains

| Chain | ID | Notes |
|-------|----|-------|
| Polygon mainnet | 137 | Full support (Safe + Proxy) |
| Amoy testnet | 80002 | Safe only (Proxy not available) |

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
