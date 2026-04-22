# Keychain Credential Storage Design

**Date**: 2026-04-16
**Status**: Approved

## Problem

Polyoxide credentials (private keys, API keys, secrets, passphrases) are loaded from environment variables or JSON files. The OS keychain is a more secure and ergonomic alternative that avoids secrets in shell history, dotfiles, and plaintext config files.

## Decisions

- **Crate**: [`keyring`](https://crates.io/crates/keyring) v3 for cross-platform keychain access (macOS Keychain, Windows Credential Manager, Linux Secret Service).
- **Scope**: All credential-bearing code — `Account`, `ApiCredentials`, `BuilderAccount`, and the CLI.
- **Location**: Shared `keychain` module in `polyoxide-core`, thin wrappers in each downstream crate.
- **Feature-gated**: Behind `keychain` feature flag, off by default.
- **Read + write**: `from_keychain()` factories to load, `save_to_keychain()` methods and standalone save functions to store.
- **Explicit source selection**: No automatic fallback chain. Users choose `from_env()`, `from_file()`, or `from_keychain()`. The CLI uses `--credential-source` to select.

## Architecture

### Core module (`polyoxide-core/src/keychain.rs`)

Behind `#[cfg(feature = "keychain")]`. Two functions and one error type:

```rust
pub fn get(service: &str, key: &str) -> Result<String, KeychainError>;
pub fn set(service: &str, key: &str, value: &str) -> Result<(), KeychainError>;

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error("Keychain entry not found: service={service}, key={key}")]
    NotFound { service: String, key: String },
    #[error("Keychain error: {0}")]
    Backend(#[from] keyring::Error),
}
```

`get()` maps `keyring::Error::NoEntry` to the typed `NotFound` variant; all other keyring errors pass through as `Backend`.

### Service and key naming

Each crate uses its own service name to namespace credentials:

| Service | Keys |
|---|---|
| `polyoxide-clob` | `private_key`, `api_key`, `api_secret`, `api_passphrase` |
| `polyoxide-relay` | `private_key`, `api_key`, `api_secret`, `passphrase`, `relayer_api_key`, `relayer_api_key_address` |

### CLOB crate (`polyoxide-clob`)

New methods on `Account` (in `account/mod.rs`), gated behind `#[cfg(feature = "keychain")]`:

- `Account::from_keychain() -> Result<Self, ClobError>` — reads all 4 keys from `polyoxide-clob` service, constructs via `Self::new()`.
- `Account::save_to_keychain(&self) -> Result<(), ClobError>` — saves the 3 L2 credential fields (key, secret, passphrase). Cannot save private key because `Account::new()` discards the raw string after parsing into `PrivateKeySigner`.
- `save_private_key_to_keychain(private_key: &str) -> Result<(), ClobError>` — standalone function for storing the private key before it gets consumed by `Account::new()`.

New method on `ApiCredentials` (in `ws/auth.rs`):

- `ApiCredentials::from_keychain() -> Result<Self, KeychainError>` — reads the same 3 L2 keys from `polyoxide-clob` (they are the same credentials as `Credentials`).

### Relay crate (`polyoxide-relay`)

New methods on `BuilderAccount` (in `account.rs`), gated behind `#[cfg(feature = "keychain")]`:

- `BuilderAccount::from_keychain() -> Result<Self, RelayError>` — reads `private_key` + optionally `api_key`/`api_secret`/`passphrase` from `polyoxide-relay` service. If `api_key` is not found, constructs with `config: None`. Passphrase is optional (matching `BuilderConfig.passphrase: Option<String>`).
- `BuilderAccount::from_keychain_relayer_api_key() -> Result<Self, RelayError>` — reads `private_key`, `relayer_api_key`, `relayer_api_key_address`.

Standalone save functions:

- `save_private_key_to_keychain(private_key: &str) -> Result<(), RelayError>`
- `save_builder_config_to_keychain(config: &BuilderConfig) -> Result<(), RelayError>`

### CLI (`polyoxide-cli`)

**New `credentials` subcommand** with two sub-subcommands:

- `polyoxide credentials store clob --private-key ... --api-key ... --api-secret ... --api-passphrase ...` — writes each provided value to the `polyoxide-clob` keychain service. Each flag is optional so individual keys can be updated.
- `polyoxide credentials store relay --private-key ... --api-key ... --api-secret ... [--passphrase ...]` — writes to `polyoxide-relay`.
- `polyoxide credentials store relay --private-key ... --relayer-api-key ... --relayer-api-key-address ...` — alternative relay auth.
- `polyoxide credentials show clob` / `polyoxide credentials show relay` — lists which keys are present/absent without printing values.

**`ws user` enhancement** — new `--credential-source` flag:

```
#[arg(long, value_enum)]
credential_source: Option<CredentialSource>,
```

`CredentialSource::Keychain` calls `ApiCredentials::from_keychain()`. Default/`Env` preserves current behavior (flags > env vars).

### Feature flag wiring

```
polyoxide-core:   keychain = ["dep:keyring"]
polyoxide-clob:   keychain = ["polyoxide-core/keychain"]
polyoxide-relay:  keychain = ["polyoxide-core/keychain"]
polyoxide:        keychain = ["polyoxide-clob/keychain"]
polyoxide-cli:    keychain = ["polyoxide-clob/keychain"]
```

`keyring = "3"` added to workspace dependencies. Not in any crate's `default` features.

### Crates not affected

`polyoxide-gamma`, `polyoxide-data`, `polyoxide-py` — no credentials, no changes.

## Error handling

Each crate maps `KeychainError` into its own error type:

- `polyoxide-clob`: `ClobError::validation(format!("Keychain error for {key}: {e}"))`
- `polyoxide-relay`: `RelayError::Api(format!("Keychain error for {key}: {e}"))`

No new error variants are added — keychain failures are surfaced through existing validation/API error paths.

## Testing

Keychain tests cannot run in CI (no keychain daemon). Tests will be `#[ignore]`-gated like existing live API tests, runnable locally with `-- --ignored`. Mock tests are not practical since `keyring` talks to system services.
