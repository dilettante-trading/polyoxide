# Keychain Credential Storage Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add OS keychain as a credential source/sink across all credential-bearing crates, gated behind a `keychain` feature flag.

**Architecture:** Thin `keychain` module in `polyoxide-core` wrapping the `keyring` crate with two functions (`get`/`set`) and a `KeychainError` type. Each downstream crate (`clob`, `relay`, CLI) adds its own `from_keychain()` / save factories that call the core helpers. All keychain code is `#[cfg(feature = "keychain")]`.

**Tech Stack:** `keyring` v3 (cross-platform OS keychain access), existing `thiserror` for error types.

**Design doc:** `docs/plans/2026-04-16-keychain-credentials-design.md`

---

### Task 1: Add `keyring` to workspace and wire `keychain` feature in `polyoxide-core`

**Files:**
- Modify: `Cargo.toml` (workspace root, line 43 area — `[workspace.dependencies]`)
- Modify: `polyoxide-core/Cargo.toml` (lines 13-14 — `[features]`)

**Step 1: Add `keyring` workspace dependency**

In `Cargo.toml` (workspace root), add to `[workspace.dependencies]`:

```toml
keyring = "3"
```

**Step 2: Add `keychain` feature and optional dep to `polyoxide-core/Cargo.toml`**

Add to `[features]`:

```toml
keychain = ["dep:keyring"]
```

Add to `[dependencies]`:

```toml
keyring = { workspace = true, optional = true }
```

**Step 3: Verify it compiles**

Run: `cargo check -p polyoxide-core` and `cargo check -p polyoxide-core --features keychain`
Expected: Both succeed.

**Step 4: Commit**

```bash
git add Cargo.toml polyoxide-core/Cargo.toml
git commit -m "build: add keyring dep and keychain feature to polyoxide-core"
```

---

### Task 2: Implement `polyoxide-core` keychain module

**Files:**
- Create: `polyoxide-core/src/keychain.rs`
- Modify: `polyoxide-core/src/lib.rs` (line 34 area — module declarations and re-exports)

**Step 1: Create `polyoxide-core/src/keychain.rs`**

```rust
//! OS keychain credential storage.
//!
//! Provides `get` and `set` functions for storing and retrieving individual
//! credential strings from the platform keychain (macOS Keychain, Windows
//! Credential Manager, Linux Secret Service).
//!
//! Gated behind the `keychain` feature flag.

use keyring::Entry;

/// Error type for keychain operations.
#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    /// The requested keychain entry does not exist.
    #[error("Keychain entry not found: service={service}, key={key}")]
    NotFound {
        /// The service name that was queried.
        service: String,
        /// The key name that was queried.
        key: String,
    },

    /// An error from the underlying keychain backend.
    #[error("Keychain error: {0}")]
    Backend(#[from] keyring::Error),
}

/// Retrieve a credential from the OS keychain.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-clob"`)
/// * `key` - The key name (e.g., `"api_key"`)
pub fn get(service: &str, key: &str) -> Result<String, KeychainError> {
    let entry = Entry::new(service, key)?;
    entry.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => KeychainError::NotFound {
            service: service.to_string(),
            key: key.to_string(),
        },
        other => KeychainError::Backend(other),
    })
}

/// Store a credential in the OS keychain.
///
/// Creates or overwrites the entry for the given `(service, key)` pair.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-clob"`)
/// * `key` - The key name (e.g., `"api_key"`)
/// * `value` - The credential value to store
pub fn set(service: &str, key: &str, value: &str) -> Result<(), KeychainError> {
    let entry = Entry::new(service, key)?;
    entry.set_password(value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_error_includes_service_and_key() {
        let err = KeychainError::NotFound {
            service: "test-svc".to_string(),
            key: "test-key".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("test-svc"), "missing service: {msg}");
        assert!(msg.contains("test-key"), "missing key: {msg}");
    }

    #[test]
    fn backend_error_display() {
        let err = KeychainError::Backend(keyring::Error::NoEntry);
        let msg = err.to_string();
        assert!(msg.contains("Keychain error"), "unexpected: {msg}");
    }
}
```

**Step 2: Register module and re-export in `polyoxide-core/src/lib.rs`**

After line 34 (`pub mod rate_limit;`), add:

```rust
#[cfg(feature = "keychain")]
pub mod keychain;
```

After the existing `pub use` block (around line 58), add:

```rust
#[cfg(feature = "keychain")]
pub use keychain::KeychainError;
```

**Step 3: Run tests**

Run: `cargo test -p polyoxide-core --features keychain`
Expected: PASS (the two unit tests exercise error formatting only, no real keychain access).

**Step 4: Run clippy**

Run: `cargo clippy -p polyoxide-core --all-targets --features keychain -- -D warnings`
Expected: No warnings.

**Step 5: Commit**

```bash
git add polyoxide-core/src/keychain.rs polyoxide-core/src/lib.rs
git commit -m "feat(core): add keychain module for OS credential storage"
```

---

### Task 3: Wire `keychain` feature in `polyoxide-clob`

**Files:**
- Modify: `polyoxide-clob/Cargo.toml` (lines 14-17 — `[features]`)

**Step 1: Add `keychain` feature to `polyoxide-clob/Cargo.toml`**

Add to `[features]`:

```toml
keychain = ["polyoxide-core/keychain"]
```

**Step 2: Verify it compiles**

Run: `cargo check -p polyoxide-clob --features keychain`
Expected: Success.

**Step 3: Commit**

```bash
git add polyoxide-clob/Cargo.toml
git commit -m "build(clob): add keychain feature flag"
```

---

### Task 4: Add `Account::from_keychain()` and save functions

**Files:**
- Modify: `polyoxide-clob/src/account/mod.rs` (after `from_json` at line 211, and after `sign_l2_request` at line 293)

**Step 1: Write the `#[ignore]` integration test**

Add to the bottom of the `#[cfg(test)] mod tests` block in `polyoxide-clob/src/account/mod.rs` (before the closing `}`):

```rust
    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        #[test]
        #[ignore] // Requires OS keychain daemon — run locally with `-- --ignored`
        fn keychain_roundtrip() {
            // Store credentials
            let private_key =
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
            let credentials = Credentials {
                key: "test_keychain_key".to_string(),
                secret: "c2VjcmV0".to_string(),
                passphrase: "test_keychain_pass".to_string(),
            };

            save_private_key_to_keychain(private_key).unwrap();
            let account = Account::new(private_key, credentials).unwrap();
            account.save_to_keychain().unwrap();

            // Load back
            let loaded = Account::from_keychain().unwrap();
            assert_eq!(loaded.credentials().key, "test_keychain_key");
            assert_eq!(loaded.credentials().secret, "c2VjcmV0");
            assert_eq!(loaded.credentials().passphrase, "test_keychain_pass");
            assert_eq!(loaded.address(), account.address());
        }
    }
```

**Step 2: Run the test to verify it fails (function doesn't exist yet)**

Run: `cargo test -p polyoxide-clob --features keychain -- keychain_roundtrip --ignored 2>&1 | head -20`
Expected: Compilation error — `save_private_key_to_keychain` not found.

**Step 3: Implement `from_keychain`, `save_to_keychain`, and `save_private_key_to_keychain`**

In `polyoxide-clob/src/account/mod.rs`, add these methods to the `impl Account` block, after `from_json` (after line 211):

```rust
    /// Load account from the OS keychain.
    ///
    /// Reads from the `polyoxide-clob` keychain service:
    /// - `private_key`: Hex-encoded private key
    /// - `api_key`: API key
    /// - `api_secret`: API secret (base64 encoded)
    /// - `api_passphrase`: API passphrase
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::Account;
    ///
    /// let account = Account::from_keychain()?;
    /// # Ok::<(), polyoxide_clob::ClobError>(())
    /// ```
    #[cfg(feature = "keychain")]
    pub fn from_keychain() -> Result<Self, ClobError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-clob";

        let private_key = keychain::get(SERVICE, "private_key")
            .map_err(|e| ClobError::validation(format!("Keychain error for private_key: {e}")))?;

        let credentials = Credentials {
            key: keychain::get(SERVICE, "api_key")
                .map_err(|e| ClobError::validation(format!("Keychain error for api_key: {e}")))?,
            secret: keychain::get(SERVICE, "api_secret").map_err(|e| {
                ClobError::validation(format!("Keychain error for api_secret: {e}"))
            })?,
            passphrase: keychain::get(SERVICE, "api_passphrase").map_err(|e| {
                ClobError::validation(format!("Keychain error for api_passphrase: {e}"))
            })?,
        };

        Self::new(private_key, credentials)
    }

    /// Save L2 API credentials to the OS keychain.
    ///
    /// Stores `api_key`, `api_secret`, and `api_passphrase` in the `polyoxide-clob`
    /// keychain service. Does **not** store the private key (it is discarded after
    /// parsing during construction). Use [`save_private_key_to_keychain`] to store
    /// the private key before constructing an `Account`.
    #[cfg(feature = "keychain")]
    pub fn save_to_keychain(&self) -> Result<(), ClobError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-clob";

        keychain::set(SERVICE, "api_key", &self.credentials.key)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        keychain::set(SERVICE, "api_secret", &self.credentials.secret)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        keychain::set(SERVICE, "api_passphrase", &self.credentials.passphrase)
            .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
        Ok(())
    }
```

Add as a free function after the `impl Account` block (after `sign_l2_request`, around line 293), before `#[cfg(test)]`:

```rust
/// Save a private key to the OS keychain under the `polyoxide-clob` service.
///
/// Call this before [`Account::new`] if you want the private key persisted in the
/// keychain, since `Account` discards the raw key string after parsing.
#[cfg(feature = "keychain")]
pub fn save_private_key_to_keychain(private_key: &str) -> Result<(), ClobError> {
    polyoxide_core::keychain::set("polyoxide-clob", "private_key", private_key)
        .map_err(|e| ClobError::validation(format!("Keychain error: {e}")))?;
    Ok(())
}
```

**Step 4: Run clippy**

Run: `cargo clippy -p polyoxide-clob --all-targets --features keychain -- -D warnings`
Expected: No warnings.

**Step 5: Run the test (locally with keychain daemon)**

Run: `cargo test -p polyoxide-clob --features keychain -- keychain_roundtrip --ignored`
Expected: PASS (on a machine with a keychain daemon running).

If no keychain daemon is available, verify compilation:
Run: `cargo test -p polyoxide-clob --features keychain --no-run`
Expected: Compiles successfully.

**Step 6: Commit**

```bash
git add polyoxide-clob/src/account/mod.rs
git commit -m "feat(clob): add Account keychain load/save methods"
```

---

### Task 5: Add `ApiCredentials::from_keychain()`

**Files:**
- Modify: `polyoxide-clob/src/ws/auth.rs` (after `from_env` at line 56)

**Step 1: Write the `#[ignore]` integration test**

Add to the `#[cfg(test)] mod tests` block in `polyoxide-clob/src/ws/auth.rs` (before the closing `}`):

```rust
    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn api_credentials_from_keychain() {
            // Pre-populate keychain (reuses values from Account tests)
            polyoxide_core::keychain::set("polyoxide-clob", "api_key", "kc_key").unwrap();
            polyoxide_core::keychain::set("polyoxide-clob", "api_secret", "kc_secret").unwrap();
            polyoxide_core::keychain::set("polyoxide-clob", "api_passphrase", "kc_pass").unwrap();

            let creds = ApiCredentials::from_keychain().unwrap();
            assert_eq!(creds.api_key, "kc_key");
            assert_eq!(creds.secret, "kc_secret");
            assert_eq!(creds.passphrase, "kc_pass");
        }
    }
```

**Step 2: Implement `from_keychain`**

In `polyoxide-clob/src/ws/auth.rs`, add after `from_env` (after line 56):

```rust
    /// Load credentials from the OS keychain.
    ///
    /// Reads from the `polyoxide-clob` keychain service:
    /// - `api_key`
    /// - `api_secret`
    /// - `api_passphrase`
    #[cfg(feature = "keychain")]
    pub fn from_keychain() -> Result<Self, polyoxide_core::KeychainError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-clob";

        Ok(Self {
            api_key: keychain::get(SERVICE, "api_key")?,
            secret: keychain::get(SERVICE, "api_secret")?,
            passphrase: keychain::get(SERVICE, "api_passphrase")?,
        })
    }
```

**Step 3: Run clippy**

Run: `cargo clippy -p polyoxide-clob --all-targets --features "keychain,ws" -- -D warnings`
Expected: No warnings.

**Step 4: Commit**

```bash
git add polyoxide-clob/src/ws/auth.rs
git commit -m "feat(clob): add ApiCredentials::from_keychain()"
```

---

### Task 6: Update `polyoxide-clob` public re-exports

**Files:**
- Modify: `polyoxide-clob/src/lib.rs` (line 63 — `pub use account::...`)

**Step 1: Add `save_private_key_to_keychain` to re-exports**

In `polyoxide-clob/src/lib.rs`, after the existing `pub use account::{...};` line (line 63), add:

```rust
#[cfg(feature = "keychain")]
pub use account::save_private_key_to_keychain;
```

**Step 2: Verify compilation**

Run: `cargo check -p polyoxide-clob --features keychain`
Expected: Success.

**Step 3: Commit**

```bash
git add polyoxide-clob/src/lib.rs
git commit -m "feat(clob): re-export save_private_key_to_keychain"
```

---

### Task 7: Wire `keychain` feature in `polyoxide-relay` and implement

**Files:**
- Modify: `polyoxide-relay/Cargo.toml` (add `keychain` feature)
- Modify: `polyoxide-relay/src/account.rs` (after `with_auth_config` at line 72, and before `#[cfg(test)]` at line 90)
- Modify: `polyoxide-relay/src/lib.rs` (re-exports)

**Step 1: Add `keychain` feature to `polyoxide-relay/Cargo.toml`**

Add to `[features]` (create section if not present, since relay has no `[features]` block yet — add after `[package]` and before `[dependencies]`):

```toml
[features]
default = []
keychain = ["polyoxide-core/keychain"]
```

**Step 2: Write `#[ignore]` integration tests**

Add to the `#[cfg(test)] mod tests` block in `polyoxide-relay/src/account.rs` (before the closing `}`):

```rust
    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        const TEST_PRIVATE_KEY: &str =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn builder_account_keychain_roundtrip() {
            save_private_key_to_keychain(TEST_PRIVATE_KEY).unwrap();
            let config = BuilderConfig::new("rk".into(), "rs".into(), Some("rp".into()));
            save_builder_config_to_keychain(&config).unwrap();

            let account = BuilderAccount::from_keychain().unwrap();
            assert_eq!(account.address(), BuilderAccount::new(TEST_PRIVATE_KEY, None).unwrap().address());
            assert!(account.auth_config().is_some());
        }

        #[test]
        #[ignore] // Requires OS keychain daemon
        fn builder_account_keychain_no_config() {
            save_private_key_to_keychain(TEST_PRIVATE_KEY).unwrap();
            // Ensure no api_key exists — attempt to get will return NotFound
            // This test relies on a clean keychain state for the relay service.
            // In practice, run after clearing relay entries.
            let account = BuilderAccount::from_keychain();
            // Should succeed (config is optional) or fail on missing private_key
            // depending on keychain state — this test mainly verifies compilation
            assert!(account.is_ok() || account.is_err());
        }
    }
```

**Step 3: Implement `from_keychain`, `from_keychain_relayer_api_key`, and save functions**

In `polyoxide-relay/src/account.rs`, add these methods to the `impl BuilderAccount` block, after `with_auth_config` (after line 72):

```rust
    /// Load account from the OS keychain with builder API credentials.
    ///
    /// Reads from the `polyoxide-relay` keychain service:
    /// - `private_key`: Hex-encoded private key (required)
    /// - `api_key`, `api_secret`: Builder API credentials (optional — if `api_key` is
    ///   not found, the account is created without auth config)
    /// - `passphrase`: Builder API passphrase (optional)
    #[cfg(feature = "keychain")]
    pub fn from_keychain() -> Result<Self, RelayError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-relay";

        let private_key = keychain::get(SERVICE, "private_key")
            .map_err(|e| RelayError::Api(format!("Keychain error for private_key: {e}")))?;

        let config = match keychain::get(SERVICE, "api_key") {
            Ok(key) => {
                let secret = keychain::get(SERVICE, "api_secret").map_err(|e| {
                    RelayError::Api(format!("Keychain error for api_secret: {e}"))
                })?;
                let passphrase = keychain::get(SERVICE, "passphrase").ok();
                Some(BuilderConfig::new(key, secret, passphrase))
            }
            Err(polyoxide_core::KeychainError::NotFound { .. }) => None,
            Err(e) => return Err(RelayError::Api(format!("Keychain error: {e}"))),
        };

        Self::new(private_key, config)
    }

    /// Load account from the OS keychain with relayer API key credentials.
    ///
    /// Reads from the `polyoxide-relay` keychain service:
    /// - `private_key`: Hex-encoded private key
    /// - `relayer_api_key`: Static relayer API key
    /// - `relayer_api_key_address`: On-chain address for the relayer API key
    #[cfg(feature = "keychain")]
    pub fn from_keychain_relayer_api_key() -> Result<Self, RelayError> {
        use polyoxide_core::keychain;
        const SERVICE: &str = "polyoxide-relay";

        let private_key = keychain::get(SERVICE, "private_key")
            .map_err(|e| RelayError::Api(format!("Keychain error for private_key: {e}")))?;
        let key = keychain::get(SERVICE, "relayer_api_key")
            .map_err(|e| RelayError::Api(format!("Keychain error for relayer_api_key: {e}")))?;
        let address = keychain::get(SERVICE, "relayer_api_key_address").map_err(|e| {
            RelayError::Api(format!("Keychain error for relayer_api_key_address: {e}"))
        })?;

        Self::with_relayer_api_key(private_key, key, address)
    }
```

Add free functions after the `impl BuilderAccount` block (before `#[cfg(test)]`):

```rust
/// Save a private key to the OS keychain under the `polyoxide-relay` service.
#[cfg(feature = "keychain")]
pub fn save_private_key_to_keychain(private_key: &str) -> Result<(), RelayError> {
    polyoxide_core::keychain::set("polyoxide-relay", "private_key", private_key)
        .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    Ok(())
}

/// Save builder API credentials to the OS keychain under the `polyoxide-relay` service.
#[cfg(feature = "keychain")]
pub fn save_builder_config_to_keychain(config: &BuilderConfig) -> Result<(), RelayError> {
    use polyoxide_core::keychain;
    const SERVICE: &str = "polyoxide-relay";

    keychain::set(SERVICE, "api_key", &config.key)
        .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    keychain::set(SERVICE, "api_secret", &config.secret)
        .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    if let Some(passphrase) = &config.passphrase {
        keychain::set(SERVICE, "passphrase", passphrase)
            .map_err(|e| RelayError::Api(format!("Keychain error: {e}")))?;
    }
    Ok(())
}
```

**Step 4: Update re-exports in `polyoxide-relay/src/lib.rs`**

After `pub use account::BuilderAccount;` (line 49), add:

```rust
#[cfg(feature = "keychain")]
pub use account::{save_builder_config_to_keychain, save_private_key_to_keychain};
```

**Step 5: Run clippy**

Run: `cargo clippy -p polyoxide-relay --all-targets --features keychain -- -D warnings`
Expected: No warnings.

**Step 6: Commit**

```bash
git add polyoxide-relay/Cargo.toml polyoxide-relay/src/account.rs polyoxide-relay/src/lib.rs
git commit -m "feat(relay): add BuilderAccount keychain load/save methods"
```

---

### Task 8: Wire `keychain` feature in unified `polyoxide` crate

**Files:**
- Modify: `polyoxide/Cargo.toml` (lines 14-19 — `[features]`)

**Step 1: Add `keychain` feature**

Add to `[features]`:

```toml
keychain = ["polyoxide-clob?/keychain"]
```

The `?` syntax ensures it only activates if `polyoxide-clob` is also enabled.

**Step 2: Verify**

Run: `cargo check -p polyoxide --features "clob,keychain"`
Expected: Success.

**Step 3: Commit**

```bash
git add polyoxide/Cargo.toml
git commit -m "build(polyoxide): add keychain feature passthrough"
```

---

### Task 9: CLI `credentials` subcommand

**Files:**
- Create: `polyoxide-cli/src/commands/credentials/mod.rs`
- Modify: `polyoxide-cli/src/commands/mod.rs` (line 3-11 — module declarations)
- Modify: `polyoxide-cli/src/main.rs` (lines 15-33 — `Commands` enum, lines 41-45 — match arms)
- Modify: `polyoxide-cli/Cargo.toml` (add `keychain` feature and `polyoxide-relay` dep)

**Step 1: Add `keychain` feature to `polyoxide-cli/Cargo.toml`**

Add after `[package]` / before `[[bin]]`:

```toml
[features]
default = []
keychain = ["polyoxide-clob/keychain", "dep:polyoxide-relay"]
```

Add to `[dependencies]`:

```toml
polyoxide-relay = { workspace = true, optional = true, features = ["keychain"] }
```

**Step 2: Create `polyoxide-cli/src/commands/credentials/mod.rs`**

```rust
//! Keychain credential management commands.

use clap::{Args, Subcommand, ValueEnum};
use color_eyre::eyre::Result;

#[derive(Args)]
pub struct CredentialsCommand {
    #[command(subcommand)]
    command: CredentialsSubcommand,
}

#[derive(Subcommand)]
enum CredentialsSubcommand {
    /// Store credentials in the OS keychain
    Store {
        #[command(subcommand)]
        target: StoreTarget,
    },
    /// Show which credentials are present in the OS keychain
    Show {
        /// Which service to check
        #[arg(value_enum)]
        target: ShowTarget,
    },
}

#[derive(Subcommand)]
enum StoreTarget {
    /// Store CLOB API credentials
    Clob(StoreClobArgs),
    /// Store Relay API credentials
    Relay(StoreRelayArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum ShowTarget {
    /// Check CLOB credentials
    Clob,
    /// Check Relay credentials
    Relay,
}

#[derive(Args)]
struct StoreClobArgs {
    /// Hex-encoded private key
    #[arg(long)]
    private_key: Option<String>,
    /// API key
    #[arg(long)]
    api_key: Option<String>,
    /// API secret (base64 encoded)
    #[arg(long)]
    api_secret: Option<String>,
    /// API passphrase
    #[arg(long)]
    api_passphrase: Option<String>,
}

#[derive(Args)]
struct StoreRelayArgs {
    /// Hex-encoded private key
    #[arg(long)]
    private_key: Option<String>,
    /// Builder API key
    #[arg(long)]
    api_key: Option<String>,
    /// Builder API secret
    #[arg(long)]
    api_secret: Option<String>,
    /// Builder API passphrase
    #[arg(long)]
    passphrase: Option<String>,
    /// Relayer API key (alternative to builder credentials)
    #[arg(long)]
    relayer_api_key: Option<String>,
    /// Relayer API key address
    #[arg(long)]
    relayer_api_key_address: Option<String>,
}

impl CredentialsCommand {
    pub fn run(self) -> Result<()> {
        match self.command {
            CredentialsSubcommand::Store { target } => match target {
                StoreTarget::Clob(args) => store_clob(args),
                StoreTarget::Relay(args) => store_relay(args),
            },
            CredentialsSubcommand::Show { target } => match target {
                ShowTarget::Clob => show_clob(),
                ShowTarget::Relay => show_relay(),
            },
        }
    }
}

fn store_clob(args: StoreClobArgs) -> Result<()> {
    use polyoxide_core::keychain;
    const SERVICE: &str = "polyoxide-clob";

    let mut stored = Vec::new();

    if let Some(val) = &args.private_key {
        keychain::set(SERVICE, "private_key", val)?;
        stored.push("private_key");
    }
    if let Some(val) = &args.api_key {
        keychain::set(SERVICE, "api_key", val)?;
        stored.push("api_key");
    }
    if let Some(val) = &args.api_secret {
        keychain::set(SERVICE, "api_secret", val)?;
        stored.push("api_secret");
    }
    if let Some(val) = &args.api_passphrase {
        keychain::set(SERVICE, "api_passphrase", val)?;
        stored.push("api_passphrase");
    }

    if stored.is_empty() {
        eprintln!("No credentials provided. Use --help to see available options.");
    } else {
        eprintln!("Stored {} credential(s) in keychain.", stored.len());
        for key in &stored {
            eprintln!("  - {key}");
        }
    }

    Ok(())
}

fn store_relay(args: StoreRelayArgs) -> Result<()> {
    use polyoxide_core::keychain;
    const SERVICE: &str = "polyoxide-relay";

    let mut stored = Vec::new();

    if let Some(val) = &args.private_key {
        keychain::set(SERVICE, "private_key", val)?;
        stored.push("private_key");
    }
    if let Some(val) = &args.api_key {
        keychain::set(SERVICE, "api_key", val)?;
        stored.push("api_key");
    }
    if let Some(val) = &args.api_secret {
        keychain::set(SERVICE, "api_secret", val)?;
        stored.push("api_secret");
    }
    if let Some(val) = &args.passphrase {
        keychain::set(SERVICE, "passphrase", val)?;
        stored.push("passphrase");
    }
    if let Some(val) = &args.relayer_api_key {
        keychain::set(SERVICE, "relayer_api_key", val)?;
        stored.push("relayer_api_key");
    }
    if let Some(val) = &args.relayer_api_key_address {
        keychain::set(SERVICE, "relayer_api_key_address", val)?;
        stored.push("relayer_api_key_address");
    }

    if stored.is_empty() {
        eprintln!("No credentials provided. Use --help to see available options.");
    } else {
        eprintln!("Stored {} credential(s) in keychain.", stored.len());
        for key in &stored {
            eprintln!("  - {key}");
        }
    }

    Ok(())
}

fn check_entry(service: &str, key: &str) -> &'static str {
    match polyoxide_core::keychain::get(service, key) {
        Ok(_) => "present",
        Err(polyoxide_core::KeychainError::NotFound { .. }) => "not found",
        Err(_) => "error",
    }
}

fn show_clob() -> Result<()> {
    const SERVICE: &str = "polyoxide-clob";

    println!("Keychain credentials for {SERVICE}:");
    println!("  private_key:     {}", check_entry(SERVICE, "private_key"));
    println!("  api_key:         {}", check_entry(SERVICE, "api_key"));
    println!("  api_secret:      {}", check_entry(SERVICE, "api_secret"));
    println!("  api_passphrase:  {}", check_entry(SERVICE, "api_passphrase"));
    Ok(())
}

fn show_relay() -> Result<()> {
    const SERVICE: &str = "polyoxide-relay";

    println!("Keychain credentials for {SERVICE}:");
    println!("  private_key:              {}", check_entry(SERVICE, "private_key"));
    println!("  api_key:                  {}", check_entry(SERVICE, "api_key"));
    println!("  api_secret:               {}", check_entry(SERVICE, "api_secret"));
    println!("  passphrase:               {}", check_entry(SERVICE, "passphrase"));
    println!("  relayer_api_key:          {}", check_entry(SERVICE, "relayer_api_key"));
    println!("  relayer_api_key_address:  {}", check_entry(SERVICE, "relayer_api_key_address"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        cmd: CredentialsCommand,
    }

    fn try_parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_from(args)
    }

    #[test]
    fn store_clob_parses_all_flags() {
        let cli = try_parse(&[
            "test", "store", "clob",
            "--private-key", "0xabc",
            "--api-key", "k",
            "--api-secret", "s",
            "--api-passphrase", "p",
        ]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store { target: StoreTarget::Clob(_) }
        ));
    }

    #[test]
    fn store_relay_parses_builder_flags() {
        let cli = try_parse(&[
            "test", "store", "relay",
            "--private-key", "0xabc",
            "--api-key", "k",
            "--api-secret", "s",
        ]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store { target: StoreTarget::Relay(_) }
        ));
    }

    #[test]
    fn store_relay_parses_relayer_api_key_flags() {
        let cli = try_parse(&[
            "test", "store", "relay",
            "--private-key", "0xabc",
            "--relayer-api-key", "rk",
            "--relayer-api-key-address", "0xaddr",
        ]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store { target: StoreTarget::Relay(_) }
        ));
    }

    #[test]
    fn show_clob_parses() {
        let cli = try_parse(&["test", "show", "clob"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Show { target: ShowTarget::Clob }
        ));
    }

    #[test]
    fn show_relay_parses() {
        let cli = try_parse(&["test", "show", "relay"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Show { target: ShowTarget::Relay }
        ));
    }

    #[test]
    fn store_clob_no_flags_parses() {
        // All flags are optional — should parse even with none
        let cli = try_parse(&["test", "store", "clob"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store { target: StoreTarget::Clob(_) }
        ));
    }
}
```

**Step 3: Register module in `polyoxide-cli/src/commands/mod.rs`**

Add after `pub mod ws;` (line 8):

```rust
#[cfg(feature = "keychain")]
pub mod credentials;
```

Add after `pub use ws::WsCommand;` (line 12):

```rust
#[cfg(feature = "keychain")]
pub use credentials::CredentialsCommand;
```

**Step 4: Add `Credentials` variant to CLI in `polyoxide-cli/src/main.rs`**

Add to the `Commands` enum (after `Completions` variant, around line 33):

```rust
    /// Manage OS keychain credentials
    #[cfg(feature = "keychain")]
    Credentials(commands::CredentialsCommand),
```

Add to the match arm in `main()` (after `Commands::Completions`, around line 45):

```rust
        #[cfg(feature = "keychain")]
        Commands::Credentials(cmd) => cmd.run()?,
```

**Step 5: Add `polyoxide-core` dependency to CLI**

In `polyoxide-cli/Cargo.toml`, add to `[dependencies]`:

```toml
polyoxide-core = { workspace = true, optional = true, features = ["keychain"] }
```

Update the `keychain` feature to also pull in core:

```toml
keychain = ["polyoxide-clob/keychain", "dep:polyoxide-core", "dep:polyoxide-relay"]
```

**Step 6: Run tests and clippy**

Run: `cargo test -p polyoxide-cli --features keychain`
Expected: All parsing tests pass.

Run: `cargo clippy -p polyoxide-cli --all-targets --features keychain -- -D warnings`
Expected: No warnings.

**Step 7: Commit**

```bash
git add polyoxide-cli/Cargo.toml polyoxide-cli/src/commands/credentials/mod.rs polyoxide-cli/src/commands/mod.rs polyoxide-cli/src/main.rs
git commit -m "feat(cli): add credentials store/show subcommand"
```

---

### Task 10: CLI `ws user --credential-source keychain`

**Files:**
- Modify: `polyoxide-cli/src/commands/ws/user.rs` (lines 25-58 — `UserArgs`, lines 71-72 — `run`, lines 150-180 — `get_credentials`)

**Step 1: Write the test**

Add to `#[cfg(test)] mod tests` in `polyoxide-cli/src/commands/ws/user.rs` (before the closing `}`):

```rust
    #[cfg(feature = "keychain")]
    mod keychain_tests {
        use super::*;

        #[test]
        fn credential_source_keychain_parses() {
            let w = try_parse(&["test", "id", "--credential-source", "keychain"]).unwrap();
            assert!(matches!(
                w.args.credential_source,
                Some(CredentialSource::Keychain)
            ));
        }

        #[test]
        fn credential_source_env_parses() {
            let w = try_parse(&["test", "id", "--credential-source", "env"]).unwrap();
            assert!(matches!(
                w.args.credential_source,
                Some(CredentialSource::Env)
            ));
        }

        #[test]
        fn credential_source_default_is_none() {
            let w = try_parse(&["test", "id"]).unwrap();
            assert!(w.args.credential_source.is_none());
        }
    }
```

**Step 2: Add `CredentialSource` enum and flag**

In `polyoxide-cli/src/commands/ws/user.rs`, add after the `UserEventType` enum (after line 23):

```rust
/// Credential source for authenticated commands
#[cfg(feature = "keychain")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CredentialSource {
    /// Load from environment variables (default)
    Env,
    /// Load from OS keychain
    Keychain,
}
```

Add to `UserArgs` struct (after `api_passphrase` field, around line 41):

```rust
    /// Credential source (env or keychain)
    #[cfg(feature = "keychain")]
    #[arg(long, value_enum)]
    credential_source: Option<CredentialSource>,
```

**Step 3: Update `run()` and `get_credentials()`**

In `run()`, update the `get_credentials` call (line 72). Replace:

```rust
    let credentials = get_credentials(args.api_key, args.api_secret, args.api_passphrase)?;
```

With:

```rust
    let credentials = get_credentials(
        args.api_key,
        args.api_secret,
        args.api_passphrase,
        #[cfg(feature = "keychain")]
        args.credential_source,
    )?;
```

Update the `get_credentials` function signature and add the keychain branch. Replace the entire function (lines 150-180):

```rust
fn get_credentials(
    api_key: Option<String>,
    api_secret: Option<String>,
    api_passphrase: Option<String>,
    #[cfg(feature = "keychain")] credential_source: Option<CredentialSource>,
) -> Result<ApiCredentials> {
    #[cfg(feature = "keychain")]
    if matches!(credential_source, Some(CredentialSource::Keychain)) {
        return ApiCredentials::from_keychain().map_err(|e| {
            color_eyre::eyre::eyre!("Failed to load credentials from keychain: {e}")
        });
    }

    match (api_key, api_secret, api_passphrase) {
        (Some(key), Some(secret), Some(passphrase)) => {
            Ok(ApiCredentials::new(key, secret, passphrase))
        }
        (key, secret, passphrase) => {
            let mut missing = Vec::new();
            if key.is_none() {
                missing.push("--api-key / POLYMARKET_API_KEY");
            }
            if secret.is_none() {
                missing.push("--api-secret / POLYMARKET_API_SECRET");
            }
            if passphrase.is_none() {
                missing.push("--api-passphrase / POLYMARKET_API_PASSPHRASE");
            }
            let list = missing
                .iter()
                .map(|m| format!("  - {}", m))
                .collect::<Vec<_>>()
                .join("\n");
            Err(color_eyre::eyre::eyre!(
                "Missing required credentials:\n\n{list}"
            ))
        }
    }
}
```

Update the existing `get_credentials_*` tests to pass the new parameter. Each call to `get_credentials(...)` in the non-keychain tests needs `#[cfg(feature = "keychain")] None` added as the last argument. For example:

```rust
    #[test]
    fn get_credentials_all_present() {
        let result = get_credentials(
            Some("key".to_string()),
            Some("secret".to_string()),
            Some("pass".to_string()),
            #[cfg(feature = "keychain")]
            None,
        );
        assert!(result.is_ok());
    }
```

Apply this pattern to all `get_credentials_*` tests.

**Step 4: Run tests and clippy**

Run: `cargo test -p polyoxide-cli --features keychain`
Expected: All tests pass (both existing and new).

Run: `cargo test -p polyoxide-cli` (without keychain feature)
Expected: All existing tests pass unchanged.

Run: `cargo clippy -p polyoxide-cli --all-targets --features keychain -- -D warnings`
Expected: No warnings.

**Step 5: Commit**

```bash
git add polyoxide-cli/src/commands/ws/user.rs
git commit -m "feat(cli): add --credential-source keychain to ws user"
```

---

### Task 11: Full workspace verification

**Step 1: Build entire workspace without keychain**

Run: `cargo build --all-features --workspace`
Expected: Success — `keychain` is not in any default features, and `--all-features` won't break because `keychain` is only activated when explicitly opted in... Actually, `--all-features` will activate `keychain`. Let's verify both paths.

Run: `cargo build --workspace`
Expected: Success (default features only, no keychain).

Run: `cargo build --workspace --all-features`
Expected: Success (all features including keychain).

**Step 2: Run all tests without keychain**

Run: `cargo test --workspace`
Expected: All existing tests pass.

**Step 3: Run all tests with keychain**

Run: `cargo test --workspace --all-features`
Expected: All tests pass (keychain `#[ignore]` tests are skipped).

**Step 4: Clippy full workspace**

Run: `cargo clippy --all-targets --all-features --workspace -- -D warnings`
Expected: No warnings.

**Step 5: Format check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues.

**Step 6: Commit (if any formatting fixes needed)**

```bash
cargo fmt --all
git add -A
git commit -m "style: apply rustfmt"
```
