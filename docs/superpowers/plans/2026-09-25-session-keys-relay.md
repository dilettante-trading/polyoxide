# Session Keys Plan 2: Relay Deposit Wallet Dialect

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `polyoxide-relay` can derive and resolve a Deposit Wallet, sign and submit a Deposit Wallet `Batch` (approvals, redemption, any calls) with a local key or hand the typed data to an external signer, and authorize, list-poll and revoke session signers through the relayer's session-signer endpoints under Builder HMAC auth, all verified offline against py-sdk vectors.

**Architecture:** A pure `wallet.rs` holds the four CREATE2 derivations; a pure `deposit_wallet.rs` holds the `Batch` EIP-712 struct, the typed-data JSON, the calldata encoders and the relay's own copy of the session-signer envelope; `session_signers.rs` holds the request and response types and the scope validator. `RelayClient` gains an auth mode independent of a wallet key, a `DepositWallet` arm in `execute`, the params and transaction routes, and typed-data-out / signature-in pairs for every owner-signed batch.

**Tech Stack:** Rust 1.91, `alloy` 1.1 (`sol!`, `SolStruct::eip712_signing_hash`, `SolCall::abi_encode`, `SolValue::abi_encode_params`), `serde_json`, `mockito`, `uv` + `polymarket-client==0.11.0` for vector capture.

**Spec:** `docs/superpowers/specs/2026-09-25-session-keys-offline-design.md` section 3 and the "Relayer, Deposit Wallet dialect" and "Deposit Wallet address derivation" parts of the venue contract. Plan 1 (`2026-09-25-session-keys-clob-signing.md`) is merged; its `SessionSignerScope` lives in `polyoxide-core`.

**Environment note:** builds on this machine can be killed by `earlyoom` (rustc exits with signal 15 / status 254). That is environmental; rerun with `CARGO_BUILD_JOBS=4`. Never put a target dir under `/tmp`.

**Gates before every commit:** `cargo fmt --all`, then `cargo clippy -p polyoxide-core -p polyoxide-clob -p polyoxide-relay --all-targets --all-features -- -D warnings`. Before the final commit also `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` and `cargo check --workspace --all-features`. A `pub` item's doc must not intra-doc-link to a non-`pub` item.

**Testing rules that bit plan 1:** every `expect(0)` mock on a route the client calls with a query string needs `.match_query(Matcher::Any)` or it can never match and proves nothing. Every signing test compares to fixture bytes from py-sdk, never to a value the code under test computed. Doc-comment links to items that do not exist yet must be plain backticks until the item lands.

---

## File map

| File | Responsibility |
|---|---|
| `scripts/capture_session_key_vectors.py` | Modify. Also writes the relay vectors. |
| `polyoxide-relay/tests/fixtures/session_keys/relay_vectors.json`, `PROVENANCE.md` | Create. Derivations, four signed batches, submit and authorization bodies. |
| `polyoxide-core/src/session_signer.rs` | Modify. `DepositWalletRole` moves here. |
| `polyoxide-clob/src/account/target.rs` | Modify. Re-exports `DepositWalletRole` from core. |
| `polyoxide-relay/src/config.rs` | Modify. Deposit Wallet and proxy-implementation constants on `ContractConfig`. |
| `polyoxide-relay/src/wallet.rs` | Create. Pure CREATE2 derivations, `WalletKind`. |
| `polyoxide-relay/src/types.rs` | Modify. `WalletType::DepositWallet`, `TransactionState`, `GaslessTransaction`, `ExecuteParams`, an open-enum macro. |
| `polyoxide-relay/src/deposit_wallet.rs` | Create. `Call`/`Batch` EIP-712, typed-data JSON, calldata encoders, session envelope, constants. |
| `polyoxide-relay/src/session_signers.rs` | Create. Request/response types, status enums, scope validator. |
| `polyoxide-relay/src/client.rs` | Modify. Auth without a key, params/deployed/transaction routes, `resolve_wallet`, `execute` Deposit Wallet arm, typed-data/submit pairs, session-signer calls, redeem pair. |
| `polyoxide-relay/src/account.rs` | Modify. `BuilderAccount::with_signer` (any alloy signer). |
| `polyoxide-relay/src/lib.rs` | Modify. Exports and crate docs. |
| `polyoxide-relay/tests/mock_api.rs` | Modify. Mock tests. |

Public API changes for the 0.33.0 notes: `ContractConfig` gains fields (external struct literals break); `WalletType` gains `DepositWallet` (external exhaustive matches break); `BuilderAccount::signer()` returns the type-erased `&DynSigner`; `RelayClient` may exist with auth but no account, so `address()` stays `Option`.

---

### Task 1: Relay golden vectors from py-sdk

**Files:**
- Modify: `scripts/capture_session_key_vectors.py`
- Create: `polyoxide-relay/tests/fixtures/session_keys/relay_vectors.json`, `polyoxide-relay/tests/fixtures/session_keys/PROVENANCE.md`

- [ ] **Step 1: Extend the script**

The script already writes the clob fixtures from `polymarket-client==0.11.0` and asserts py-sdk's golden order digest before writing. Add a second output directory argument and this generator. Keep every existing behaviour (the golden assert, `PROVENANCE.md` for the clob dir, the dependency cutoff) unchanged; only add.

Add these imports next to the existing ones:

```python
from polymarket._internal.actions.relayer.calls import (
    authorize_session_signer_call,
    ctf_redeem_positions_call,
    erc20_approval_call,
    revoke_session_signer_call,
)
from polymarket._internal.actions.relayer.gasless import build_deposit_wallet_payload
from polymarket._internal.actions.relayer.signing.deposit_wallet import (
    build_deposit_wallet_typed_data,
    sign_deposit_wallet_batch,
)
from polymarket._internal.environment import PRODUCTION_CONFIG
from polymarket._internal.wallet import (
    derive_beacon_deposit_wallet_address,
    derive_proxy_wallet_address,
    derive_safe_wallet_address,
    derive_uups_deposit_wallet_address,
)
```

Add these constants and functions (module level):

```python
SIGNER_ONE = "0x0000000000000000000000000000000000000001"
PUSD = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
CTF = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
EXCHANGE_V2 = "0xE111180000d2663C0091e4f400237545B87B996B"
MAX_UINT256 = (1 << 256) - 1
CONDITION_ID = "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51"


def _hexify(value):
    if isinstance(value, (bytes, bytearray)):
        return "0x" + value.hex()
    if isinstance(value, dict):
        return {k: _hexify(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_hexify(v) for v in value]
    return value


def _derivations(signer: str, cfg) -> dict:
    return {
        "signer": signer,
        "uups": derive_uups_deposit_wallet_address(signer, cfg),
        "beacon": derive_beacon_deposit_wallet_address(signer, cfg),
        "safe": derive_safe_wallet_address(signer, cfg),
        "proxy": derive_proxy_wallet_address(signer, cfg),
    }


def _batch(signer, wallet: EvmAddress, calls, nonce: str, deadline: str, session: str) -> dict:
    typed_data = build_deposit_wallet_typed_data(
        wallet=wallet, calls=calls, nonce=nonce, deadline=deadline, chain_id=137
    )
    signature = sign_deposit_wallet_batch(
        signer, wallet=wallet, calls=calls, nonce=nonce, deadline=deadline, chain_id=137
    )
    return {
        "typed_data": _hexify(typed_data),
        "digest": digest(typed_data),
        "signature": signature,
        "session_signature": wrap_deposit_wallet_session_signer_signature(
            EvmAddress(session), HexString(signature)
        ),
        "calls": [{"target": str(c.to), "value": str(c.value), "data": c.data} for c in calls],
        "nonce": nonce,
        "deadline": deadline,
    }


def relay_vectors(signer) -> dict:
    cfg = PRODUCTION_CONFIG.wallet_derivation
    owner = signer.address
    wallet = EvmAddress(derive_beacon_deposit_wallet_address(owner, cfg))
    approval = erc20_approval_call(
        token_address=EvmAddress(PUSD), spender=EvmAddress(EXCHANGE_V2), amount=MAX_UINT256
    )
    authorize = authorize_session_signer_call(
        wallet_address=wallet, session_signer=EvmAddress(ANVIL_ADDR_1), valid_until=1815534000
    )
    revoke = revoke_session_signer_call(wallet_address=wallet, session_signer=EvmAddress(ANVIL_ADDR_1))
    redeem = ctf_redeem_positions_call(
        ctf=EvmAddress(CTF), collateral=EvmAddress(PUSD), condition_id=CONDITION_ID
    )
    approval_batch = _batch(signer, wallet, [approval], "3", "1800000000", ANVIL_ADDR_1)
    authorize_batch = _batch(signer, wallet, [authorize], "4", "1800000600", ANVIL_ADDR_1)
    return {
        "owner": owner,
        "wallet": wallet,
        "session_signer": ANVIL_ADDR_1,
        "chain_id": 137,
        "config": {
            "deposit_wallet_factory": cfg.deposit_wallet_factory,
            "deposit_wallet_beacon": cfg.deposit_wallet_beacon,
            "deposit_wallet_implementation": cfg.deposit_wallet_implementation,
            "proxy_factory": cfg.proxy_factory,
            "proxy_implementation": cfg.proxy_implementation,
            "safe_factory": cfg.safe_factory,
            "safe_init_code_hash": cfg.safe_init_code_hash,
        },
        "derivations": {
            "signer_one": _derivations(SIGNER_ONE, cfg),
            "anvil0": _derivations(owner, cfg),
        },
        "approval_batch": approval_batch,
        "authorize_batch": authorize_batch,
        "revoke_batch": _batch(signer, wallet, [revoke], "5", "1800000600", ANVIL_ADDR_1),
        "redeem_batch": _batch(signer, wallet, [redeem], "6", "1800000600", ANVIL_ADDR_1),
        "submit_body": build_deposit_wallet_payload(
            signer_address=owner,
            deposit_wallet_factory=cfg.deposit_wallet_factory,
            wallet=wallet,
            calls=[approval],
            nonce="3",
            deadline="1800000000",
            signature=approval_batch["signature"],
            metadata="",
        ),
        "authorization_body": {
            "deadline": "1800000600",
            "nonce": "4",
            "scopes": ["CLOB"],
            "sessionSignerAddress": ANVIL_ADDR_1,
            "signature": authorize_batch["signature"],
            "validUntil": "1815534000",
            "walletAddress": wallet,
        },
    }
```

`build_deposit_wallet_payload` returns a `RelayerEnvelope` (a dict subclass); wrap it in `dict(...)` if `json.dumps` cannot serialise it. `digest` and `wrap_deposit_wallet_session_signer_signature` are the functions the script already has. Change `main` to take two directories: `main(clob_dir: Path, relay_dir: Path)`, write `relay_vectors.json` (indent 2, trailing newline) and a `PROVENANCE.md` in `relay_dir` modelled on the clob one, stating: the generator command, the py-sdk version, the dependency cutoff, that the owner is Anvil key #0 and the wallet its beacon Deposit Wallet, that `signer_one` reproduces py-sdk's own `tests/unit/test_wallet_derivations.py` addresses, and that `session_signature` is a byte-pinning vector (key #0 signed, envelope names address #1). Update the usage docstring:

```
    uv run scripts/capture_session_key_vectors.py \
        polyoxide-clob/tests/fixtures/session_keys polyoxide-relay/tests/fixtures/session_keys
```

- [ ] **Step 2: Run it**

Run: `uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys polyoxide-relay/tests/fixtures/session_keys`
Expected: both `wrote …` lines; `git status` shows the clob fixtures **unchanged** and the two new relay files.

- [ ] **Step 3: Verify against known values**

These were produced from py-sdk 0.11.0 on 2026-09-25 and must match exactly:

| key | value |
|---|---|
| `wallet` (beacon DW of Anvil #0) | `0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50` |
| `derivations.signer_one.uups` | `0x57ffBc34De23124fAeb8387fcd689d314E57aCcD` |
| `derivations.signer_one.beacon` | `0x94bF330955A0b957662fEaF878dE77bf25f76cD9` |
| `derivations.signer_one.safe` | `0x766b6851A199BF91Ae3fa13B1cfaC5187355118f` |
| `derivations.signer_one.proxy` | `0x7754536ecd85c00b2E0CF9c1aA679340D8550756` |
| `derivations.anvil0.uups` | `0xdf8b9E8f9AB23f261F6e1B171B7454ae6E46Ba76` |
| `derivations.anvil0.safe` | `0xd93B25cb943D14d0d34FBaF01Fc93a0f8b5F6E47` |
| `derivations.anvil0.proxy` | `0x365f0CA36Ae1f641E02fE3B7743673da42A13A70` |
| `approval_batch.digest` | `0x6142217cedb047b76ab08e81bc709bef31f7afe3179bc706c79b2ebda1af39db` |
| `authorize_batch.digest` | `0xba435e27bb0258e5a631d904cb28416f163aee1532901376b30e4939de58976b` |
| `revoke_batch.digest` | `0x2856c1f8a21239cb091f2ca96709a76ab97ce55ed49e653ed83809ee0a3cd24f` |
| `redeem_batch.digest` | `0x271bfac1b0f09b942e79fcf1c5f9426876266de5f48114b15b128e7b9bcc160c` |
| `authorize_batch.calls[0].data` | starts `0x24017fae`, 138 characters long including `0x` (68 bytes) |
| `revoke_batch.calls[0].data` | starts `0xe63f952f`, 74 characters long including `0x` (36 bytes) |
| `approval_batch.calls[0].data` | starts `0x095ea7b3`, 138 characters long including `0x` (68 bytes) |
| every `signature` | 65 bytes; every `session_signature` 256 bytes ending `6492`×16 |
| `submit_body.type` / `to` | `"WALLET"` / the factory `0x00000000000Fb5C9ADea0298D729A0CB3823Cc07` |

The `signer_one` addresses equal py-sdk's `EXPECTED_*` constants in `tests/unit/test_wallet_derivations.py`, which py-sdk labels "matches ts golden vector". Add an assert for `signer_one.uups == "0x57ffBc34De23124fAeb8387fcd689d314E57aCcD"` (case-insensitive) in the script before writing, mirroring the clob golden assert.

- [ ] **Step 4: Commit**

```bash
git add scripts/capture_session_key_vectors.py polyoxide-relay/tests/fixtures/session_keys/
git commit -m "test(relay): golden vectors for Deposit Wallet batches and derivations from py-sdk"
```

---

### Task 2: `DepositWalletRole` moves to core

**Files:**
- Modify: `polyoxide-core/src/session_signer.rs`, `polyoxide-core/src/lib.rs`
- Modify: `polyoxide-clob/src/account/target.rs`

- [ ] **Step 1: Write the failing test**

Append to the tests module in `polyoxide-core/src/session_signer.rs`:

```rust
    #[test]
    fn deposit_wallet_role_is_copy_and_distinguishes_the_two_roles() {
        let owner = DepositWalletRole::Owner;
        let copy = owner;
        assert_eq!(owner, copy);
        assert_ne!(DepositWalletRole::Owner, DepositWalletRole::SessionKey);
    }
```

Run: `cargo test -p polyoxide-core session_signer` — expect a compile error.

- [ ] **Step 2: Move the enum**

In `polyoxide-core/src/session_signer.rs`, add after the module doc (before `SessionSignerScope`):

```rust
/// Whether a key acting for a Deposit Wallet is the wallet's owner or an
/// authorized session key.
///
/// A session key's signatures, for orders and for relayer batches alike, are
/// wrapped in an extra ERC-6492-style envelope naming the session signer; an
/// owner's are not. Both are EOAs distinct from the wallet, so the role cannot
/// be inferred from addresses and is stated explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepositWalletRole {
    /// The EOA that owns the Deposit Wallet; the wallet's address is derived from this key.
    Owner,
    /// A key the owner authorized through `authorizeSessionSigner`; it can trade but not withdraw.
    SessionKey,
}
```

In `polyoxide-core/src/lib.rs`, change `pub use session_signer::SessionSignerScope;` to `pub use session_signer::{DepositWalletRole, SessionSignerScope};`.

In `polyoxide-clob/src/account/target.rs`, delete the local `DepositWalletRole` enum and its doc, and add `pub use polyoxide_core::DepositWalletRole;` after the `use` lines. Every existing path (`polyoxide_clob::DepositWalletRole`, `polyoxide_clob::account::DepositWalletRole`, `account::target::DepositWalletRole`) keeps resolving through the re-export. The `SigningTarget` doc's mention of the role stays as is.

- [ ] **Step 3: Verify**

Run: `cargo test -p polyoxide-core session_signer` (3 passed) and `cargo test -p polyoxide-clob --all-features` (unchanged totals), plus the clippy and doc gates for both crates.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-core/src/session_signer.rs polyoxide-core/src/lib.rs polyoxide-clob/src/account/target.rs
git commit -m "refactor(core): DepositWalletRole lives in core for clob and relay to share"
```

---

### Task 3: Pure wallet derivations and `WalletKind`

**Files:**
- Modify: `polyoxide-relay/src/config.rs` (`ContractConfig` fields)
- Create: `polyoxide-relay/src/wallet.rs`
- Modify: `polyoxide-relay/src/client.rs` (`derive_safe_address`, `derive_proxy_wallet` delegate; the two hex constants move)
- Modify: `polyoxide-relay/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-relay/src/wallet.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::get_contract_config;
    use alloy::primitives::address;

    const VECTORS: &str = include_str!("../tests/fixtures/session_keys/relay_vectors.json");

    fn vectors() -> serde_json::Value {
        serde_json::from_str(VECTORS).unwrap()
    }

    fn expected(who: &str, kind: &str) -> Address {
        vectors()["derivations"][who][kind]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn signer(who: &str) -> Address {
        vectors()["derivations"][who]["signer"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn derivations_match_py_sdk_for_both_fixture_signers() {
        let cfg = get_contract_config(137).unwrap();
        for who in ["signer_one", "anvil0"] {
            let owner = signer(who);
            assert_eq!(derive_deposit_wallet_uups(owner, &cfg).unwrap(), expected(who, "uups"), "{who} uups");
            assert_eq!(derive_deposit_wallet_beacon(owner, &cfg).unwrap(), expected(who, "beacon"), "{who} beacon");
            assert_eq!(derive_safe(owner, &cfg), expected(who, "safe"), "{who} safe");
            assert_eq!(derive_proxy(owner, &cfg).unwrap(), expected(who, "proxy"), "{who} proxy");
        }
    }

    #[test]
    fn contract_config_matches_the_fixture_constants() {
        let cfg = get_contract_config(137).unwrap();
        let c = &vectors()["config"];
        let addr = |k: &str| -> Address { c[k].as_str().unwrap().parse().unwrap() };
        assert_eq!(cfg.deposit_wallet_factory, Some(addr("deposit_wallet_factory")));
        assert_eq!(cfg.deposit_wallet_beacon, Some(addr("deposit_wallet_beacon")));
        assert_eq!(cfg.deposit_wallet_implementation, Some(addr("deposit_wallet_implementation")));
        assert_eq!(cfg.proxy_factory, Some(addr("proxy_factory")));
        assert_eq!(cfg.proxy_implementation, Some(addr("proxy_implementation")));
        assert_eq!(cfg.safe_factory, addr("safe_factory"));
        assert_eq!(SAFE_INIT_CODE_HASH.to_string(), c["safe_init_code_hash"].as_str().unwrap());
    }

    #[test]
    fn signer_one_uups_is_the_clob_fixture_wallet() {
        // The Deposit Wallet used throughout the clob order vectors is signer 0x…01's UUPS wallet.
        let cfg = get_contract_config(137).unwrap();
        assert_eq!(
            derive_deposit_wallet_uups(address!("0000000000000000000000000000000000000001"), &cfg).unwrap(),
            address!("57ffbc34de23124faeb8387fcd689d314e57accd")
        );
    }

    #[test]
    fn deposit_wallet_derivations_need_the_factory_constants() {
        let cfg = get_contract_config(80002).unwrap();
        assert!(derive_deposit_wallet_uups(Address::ZERO, &cfg).is_err());
        assert!(derive_deposit_wallet_beacon(Address::ZERO, &cfg).is_err());
    }

    #[test]
    fn wallet_kind_reports_its_address() {
        let a = address!("0000000000000000000000000000000000000001");
        assert_eq!(WalletKind::DepositWallet(a).address(), Some(a));
        assert_eq!(WalletKind::Safe(a).address(), Some(a));
        assert_eq!(WalletKind::None.address(), None);
    }
}
```

Add `pub mod wallet;` (before `mod account;`) to `polyoxide-relay/src/lib.rs`.

Run: `cargo test -p polyoxide-relay wallet::` — expect compile errors.

- [ ] **Step 2: Extend `ContractConfig`**

In `polyoxide-relay/src/config.rs`, change the struct and both literals:

```rust
/// On-chain contract addresses and RPC configuration for a specific chain.
#[derive(Clone, Debug)]
pub struct ContractConfig {
    pub safe_factory: Address,
    pub safe_multisend: Address,
    pub proxy_factory: Option<Address>,
    /// Implementation behind every proxy wallet; part of the proxy CREATE2 init code.
    pub proxy_implementation: Option<Address>,
    pub relay_hub: Option<Address>,
    /// CREATE2 factory for Deposit Wallets (both generations).
    pub deposit_wallet_factory: Option<Address>,
    /// Implementation of the UUPS generation (wallets deployed before 2026-06-29).
    pub deposit_wallet_implementation: Option<Address>,
    /// Beacon of the ERC-1967 beacon generation (wallets deployed on or after 2026-06-29).
    pub deposit_wallet_beacon: Option<Address>,
    pub rpc_url: &'static str,
}
```

Polygon mainnet (137) literal gains:

```rust
            proxy_implementation: Some(address!("44e999d5c2F66Ef0861317f9A4805AC2e90aEB4f")),
            deposit_wallet_factory: Some(address!("00000000000Fb5C9ADea0298D729A0CB3823Cc07")),
            deposit_wallet_implementation: Some(address!("58CA52ebe0DadfdF531Cde7062e76746de4Db1eB")),
            deposit_wallet_beacon: Some(address!("7A18EDfe055488A3128f01F563e5B479D92ffc3a")),
```

Amoy (80002) literal gains the same four fields as `None`. Update `test_contract_config_polygon_mainnet` in `client.rs` (about line 1197) to also assert the factory is `Some(..)` and `test_contract_config_amoy_testnet` that it is `None`.

- [ ] **Step 3: Implement the derivations**

Prepend to `polyoxide-relay/src/wallet.rs`:

```rust
//! Pure CREATE2 derivations of the account wallets a signer may own, and the
//! resolver's answer type.
//!
//! Every function here is deterministic and network-free, pinned to addresses
//! produced by Polymarket's `py-sdk` (`tests/fixtures/session_keys/relay_vectors.json`).
//! Whether a derived wallet exists on chain is a separate question answered by
//! `RelayClient::resolve_wallet`.

use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::sol_types::SolValue;

use crate::config::ContractConfig;
use crate::error::RelayError;

/// Safe proxy init code hash (from the Polymarket relayer client constants).
pub const SAFE_INIT_CODE_HASH: B256 = alloy::primitives::b256!(
    "2bce2127ff07fb632d16c8347c4ebf501f4841168bed00d9e6ef715ddb6fcecf"
);

/// The deployed proxy-wallet bytecode with the factory and implementation
/// substituted, as `polymarket/py-sdk` `wallet.py` builds it. `{factory}` and
/// `{impl}` are replaced by the two 20-byte addresses in lowercase hex.
const PROXY_BYTECODE_TEMPLATE: &str = "3d3d606380380380913d393d73{factory}5af4602a57600080fd5b602d8060366000396000f3363d3d373d3d3d363d73{impl}5af43d82803e903d91602b57fd5bf352e831dd00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000";

// ERC-1967 minimal proxy pieces (Solady), split exactly as py-sdk splits them.
const ERC1967_CONST1: [u8; 32] = alloy::primitives::hex!("cc3735a920a3ca505d382bbc545af43d6000803e6038573d6000fd5b3d6000f3");
const ERC1967_CONST2: [u8; 32] = alloy::primitives::hex!("5155f3363d3d373d3d363d7f360894a13ba1a3210667c828492db98dca3e2076");
const ERC1967_PREFIX_BASE: u128 = 0x61003D3D8160233D3973;
const ERC1967_BEACON_CONST1: [u8; 32] = alloy::primitives::hex!("b3582b35133d50545afa5036515af43d6000803e604d573d6000fd5b3d6000f3");
const ERC1967_BEACON_CONST2: [u8; 32] = alloy::primitives::hex!("1b60e01b36527fa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6c");
const ERC1967_BEACON_CONST3: [u8; 23] = alloy::primitives::hex!("60195155f3363d3d373d3d363d602036600436635c60da");
const ERC1967_BEACON_PREFIX_BASE: u128 = 0x6100523D8160233D3973;

/// What `RelayClient::resolve_wallet` found for an owner.
///
/// A Proxy wallet is not represented here: the relayer's `/deployed` route only
/// answers for `SAFE` and `WALLET`, and a Proxy auto-deploys on first use, so it
/// cannot be observed before that. Use [`derive_proxy`] when you know the
/// account is a Proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletKind {
    /// A deployed Deposit Wallet (either generation).
    DepositWallet(Address),
    /// A deployed Gnosis Safe.
    Safe(Address),
    /// Nothing deployed for this owner.
    None,
}

impl WalletKind {
    /// The wallet address, if one is deployed.
    pub fn address(&self) -> Option<Address> {
        match self {
            Self::DepositWallet(a) | Self::Safe(a) => Some(*a),
            Self::None => None,
        }
    }
}

/// `keccak256(0xff ‖ factory ‖ salt ‖ init_code_hash)[12..]`.
fn create2(factory: Address, salt: B256, init_code_hash: B256) -> Address {
    let mut input = Vec::with_capacity(85);
    input.push(0xff);
    input.extend_from_slice(factory.as_slice());
    input.extend_from_slice(salt.as_slice());
    input.extend_from_slice(init_code_hash.as_slice());
    Address::from_slice(&keccak256(input)[12..])
}

/// The Gnosis Safe a signer owns. Salt is `keccak256(abi.encode(owner))`.
pub fn derive_safe(owner: Address, cfg: &ContractConfig) -> Address {
    let salt = keccak256(owner.abi_encode());
    create2(cfg.safe_factory, salt, SAFE_INIT_CODE_HASH)
}

/// The Proxy wallet a signer owns. Salt is `keccak256(abi.encodePacked(owner))`.
pub fn derive_proxy(owner: Address, cfg: &ContractConfig) -> Result<Address, RelayError> {
    let factory = cfg
        .proxy_factory
        .ok_or_else(|| RelayError::Api("Proxy wallet not supported on this chain".to_string()))?;
    let implementation = cfg.proxy_implementation.ok_or_else(|| {
        RelayError::Api("Proxy implementation not configured for this chain".to_string())
    })?;
    let bytecode_hex = PROXY_BYTECODE_TEMPLATE
        .replace("{factory}", &hex::encode(factory.as_slice()))
        .replace("{impl}", &hex::encode(implementation.as_slice()));
    let bytecode = hex::decode(bytecode_hex).expect("template is valid hex");
    let salt = keccak256(owner.as_slice());
    Ok(create2(factory, salt, keccak256(bytecode)))
}

/// `abi.encode(factory, bytes32(leftPad(owner)))`: the constructor args of both
/// Deposit Wallet generations, also hashed for the CREATE2 salt.
fn deposit_wallet_args(owner: Address, factory: Address) -> Vec<u8> {
    let wallet_id = B256::left_padding_from(owner.as_slice());
    (factory, wallet_id).abi_encode_params()
}

fn deposit_wallet_factory(cfg: &ContractConfig) -> Result<Address, RelayError> {
    cfg.deposit_wallet_factory.ok_or_else(|| {
        RelayError::Api("Deposit Wallets are not supported on this chain".to_string())
    })
}

/// 10-byte ERC-1967 creation-code prefix with the args length folded in.
fn prefix_bytes(base: u128, args_len: usize) -> [u8; 10] {
    let prefix = base + ((args_len as u128) << 56);
    let bytes = prefix.to_be_bytes();
    bytes[6..16].try_into().expect("10 bytes")
}

/// The UUPS-generation Deposit Wallet (deployed before 2026-06-29).
pub fn derive_deposit_wallet_uups(owner: Address, cfg: &ContractConfig) -> Result<Address, RelayError> {
    let factory = deposit_wallet_factory(cfg)?;
    let implementation = cfg.deposit_wallet_implementation.ok_or_else(|| {
        RelayError::Api("Deposit Wallet implementation not configured for this chain".to_string())
    })?;
    let args = deposit_wallet_args(owner, factory);
    let mut code = Vec::new();
    code.extend_from_slice(&prefix_bytes(ERC1967_PREFIX_BASE, args.len()));
    code.extend_from_slice(implementation.as_slice());
    code.extend_from_slice(&[0x60, 0x09]);
    code.extend_from_slice(&ERC1967_CONST2);
    code.extend_from_slice(&ERC1967_CONST1);
    code.extend_from_slice(&args);
    Ok(create2(factory, keccak256(&args), keccak256(code)))
}

/// The beacon-generation Deposit Wallet (deployed on or after 2026-06-29).
pub fn derive_deposit_wallet_beacon(owner: Address, cfg: &ContractConfig) -> Result<Address, RelayError> {
    let factory = deposit_wallet_factory(cfg)?;
    let beacon = cfg.deposit_wallet_beacon.ok_or_else(|| {
        RelayError::Api("Deposit Wallet beacon not configured for this chain".to_string())
    })?;
    let args = deposit_wallet_args(owner, factory);
    let mut code = Vec::new();
    code.extend_from_slice(&prefix_bytes(ERC1967_BEACON_PREFIX_BASE, args.len()));
    code.extend_from_slice(beacon.as_slice());
    code.extend_from_slice(&ERC1967_BEACON_CONST3);
    code.extend_from_slice(&ERC1967_BEACON_CONST2);
    code.extend_from_slice(&ERC1967_BEACON_CONST1);
    code.extend_from_slice(&args);
    Ok(create2(factory, keccak256(&args), keccak256(code)))
}

// `U256` is used by the salt arithmetic in tests only when needed; keep the import
// if a helper below needs it, otherwise remove it to satisfy clippy.
```

Remove the trailing comment and the `U256` import if unused. `hex` is already a dependency of the crate (`alloy::hex` is used in client.rs; the `hex` crate is in `Cargo.toml`). `alloy::primitives::{b256!, hex!}` are const macros in alloy-primitives 1.x. The prefix arithmetic: py-sdk computes `prefix = BASE + (args_len << 56)` as a Python int and serialises it to 10 big-endian bytes; `BASE` fits in 80 bits, so a `u128` and taking the low 10 bytes of the 16-byte big-endian encoding is equivalent.

In `polyoxide-relay/src/client.rs`: delete the `SAFE_INIT_CODE_HASH` and `PROXY_INIT_CODE_HASH` constants and the test `test_hex_constants_are_valid`; make `derive_safe_address` return `crate::wallet::derive_safe(owner, &self.contract_config)` and `derive_proxy_wallet` return `crate::wallet::derive_proxy(owner, &self.contract_config)`. The old `derive_proxy_wallet` used a hard-coded init-code hash for the same factory and implementation; the vector test in `wallet.rs` proves the computed one agrees (`anvil0.proxy`), and the existing client tests `test_derive_proxy_wallet_deterministic` and `test_safe_and_proxy_addresses_differ` still pass. Add a client test asserting `get_expected_proxy_wallet()` for `TEST_KEY` equals `0x365f0CA36Ae1f641E02fE3B7743673da42A13A70` and `get_expected_safe()` equals `0xd93B25cb943D14d0d34FBaF01Fc93a0f8b5F6E47` (both from the fixture's `anvil0`), so the delegation is pinned.

In `polyoxide-relay/src/lib.rs`, add `pub use wallet::{derive_deposit_wallet_beacon, derive_deposit_wallet_uups, derive_proxy, derive_safe, WalletKind};`.

- [ ] **Step 4: Verify**

Run: `cargo test -p polyoxide-relay` — the four wallet tests and every existing test pass. Gates.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-relay/src/config.rs polyoxide-relay/src/wallet.rs polyoxide-relay/src/client.rs polyoxide-relay/src/lib.rs
git commit -m "feat(relay): pure CREATE2 derivations for Deposit Wallet, Safe and Proxy, pinned to py-sdk"
```

---

### Task 4: Wire types and the v1 account routes

**Files:**
- Modify: `polyoxide-relay/src/types.rs` (`WalletType::DepositWallet`, open-enum macro, `TransactionState`, `GaslessTransaction`, `ExecuteParams`)
- Modify: `polyoxide-relay/src/client.rs` (`get_deployed_typed`, `get_execute_params`, `get_gasless_transaction`, `resolve_wallet`; `get_nonce` and `get_deployed` unchanged)
- Modify: `polyoxide-relay/src/lib.rs`
- Modify: `polyoxide-relay/tests/mock_api.rs`

- [ ] **Step 1: Write the failing unit tests**

Append to the tests module in `polyoxide-relay/src/types.rs`:

```rust
    #[test]
    fn wallet_type_deposit_wallet_is_wallet_on_the_wire() {
        assert_eq!(WalletType::DepositWallet.as_str(), "WALLET");
    }

    #[test]
    fn transaction_state_round_trips_and_preserves_unknowns() {
        for (state, wire) in [
            (TransactionState::New, "\"STATE_NEW\""),
            (TransactionState::Executed, "\"STATE_EXECUTED\""),
            (TransactionState::Mined, "\"STATE_MINED\""),
            (TransactionState::Confirmed, "\"STATE_CONFIRMED\""),
            (TransactionState::Invalid, "\"STATE_INVALID\""),
            (TransactionState::Failed, "\"STATE_FAILED\""),
        ] {
            assert_eq!(serde_json::to_string(&state).unwrap(), wire);
            assert_eq!(serde_json::from_str::<TransactionState>(wire).unwrap(), state);
        }
        let other: TransactionState = serde_json::from_str("\"STATE_QUEUED\"").unwrap();
        assert_eq!(other, TransactionState::Other("STATE_QUEUED".into()));
        assert!(!other.is_terminal());
        assert!(TransactionState::Confirmed.is_terminal() && TransactionState::Confirmed.is_success());
        assert!(TransactionState::Failed.is_terminal() && !TransactionState::Failed.is_success());
        assert!(TransactionState::Invalid.is_terminal() && !TransactionState::Invalid.is_success());
        assert!(!TransactionState::Mined.is_terminal());
    }

    #[test]
    fn gasless_transaction_parses_the_v1_shape() {
        let json = r#"{"transaction_id":"tx-1","transaction_hash":null,"state":"STATE_NEW","error_msg":null}"#;
        let tx: GaslessTransaction = serde_json::from_str(json).unwrap();
        assert_eq!(tx.transaction_id, "tx-1");
        assert_eq!(tx.transaction_hash, None);
        assert_eq!(tx.state, TransactionState::New);
        assert_eq!(tx.error_msg, None);

        let json = r#"{"transaction_id":"tx-2","transaction_hash":"0xab","state":"STATE_CONFIRMED","error_msg":""}"#;
        let tx: GaslessTransaction = serde_json::from_str(json).unwrap();
        assert_eq!(tx.transaction_hash.as_deref(), Some("0xab"));
        assert_eq!(tx.error_msg.as_deref(), Some(""));
    }

    #[test]
    fn execute_params_take_a_string_or_number_nonce() {
        let p: ExecuteParams = serde_json::from_str(r#"{"address":"0x0000000000000000000000000000000000000001","nonce":"7"}"#).unwrap();
        assert_eq!(p.nonce, 7);
        let p: ExecuteParams = serde_json::from_str(r#"{"address":"0x0000000000000000000000000000000000000001","nonce":8}"#).unwrap();
        assert_eq!(p.nonce, 8);
    }
```

Run: `cargo test -p polyoxide-relay types::` — expect compile errors.

- [ ] **Step 2: Implement the types**

In `polyoxide-relay/src/types.rs`:

Add the variant to `WalletType`:

```rust
    /// Deposit Wallet - Polymarket's smart account (default since 2026-05-04); `WALLET` on the wire.
    DepositWallet,
```
and the `as_str` arm `WalletType::DepositWallet => "WALLET"`.

Add a private macro and the new types (above the tests module):

```rust
/// A string enum that keeps unknown wire values instead of rejecting them.
macro_rules! open_string_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$vmeta:meta])* $variant:ident => $wire:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this crate does not know yet, kept verbatim.
            Other(String),
        }

        impl $name {
            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Other(s) => s,
                }
            }

            /// Parse a wire spelling; anything unrecognised becomes `Other`.
            pub fn from_wire(s: &str) -> Self {
                match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Other(other.to_string()),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self::from_wire(s))
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                Ok(Self::from_wire(&s))
            }
        }
    };
}
pub(crate) use open_string_enum;

open_string_enum! {
    /// Lifecycle state of a relayer transaction (`GET /v1/account/transactions/{id}`).
    ///
    /// `Confirmed` is the only success; `Failed` and `Invalid` are terminal
    /// failures; everything else is still in flight.
    TransactionState {
        /// Accepted by the relayer, not yet broadcast.
        New => "STATE_NEW",
        /// Broadcast to the chain.
        Executed => "STATE_EXECUTED",
        /// Included in a block, awaiting confirmations.
        Mined => "STATE_MINED",
        /// Confirmed on chain.
        Confirmed => "STATE_CONFIRMED",
        /// Rejected before broadcast (bad nonce, deadline, signature).
        Invalid => "STATE_INVALID",
        /// Reverted or dropped on chain.
        Failed => "STATE_FAILED",
    }
}

impl TransactionState {
    /// `true` once the relayer will not change this state again.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Confirmed | Self::Invalid | Self::Failed)
    }

    /// `true` only for [`TransactionState::Confirmed`].
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Confirmed)
    }
}

/// `GET /v1/account/transactions/{id}`: the poll record for a submitted transaction.
///
/// This is the v1 route the Deposit Wallet flows use; it is snake_case, unlike the
/// legacy `GET /transaction` record ([`RelayerTransaction`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GaslessTransaction {
    /// Relayer transaction id.
    pub transaction_id: String,
    /// On-chain hash once broadcast.
    pub transaction_hash: Option<String>,
    /// Current state.
    pub state: TransactionState,
    /// The relayer's failure reason for a terminal failure, if any.
    #[serde(default)]
    pub error_msg: Option<String>,
}

/// `GET /v1/account/transactions/params`: the next nonce for a wallet type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteParams {
    /// The address the nonce was fetched for.
    pub address: alloy::primitives::Address,
    /// Next nonce; the wire sends it as a decimal string.
    #[serde(deserialize_with = "deserialize_nonce")]
    pub nonce: u64,
}
```

`Serialize`/`Deserialize` are already imported at the top of `types.rs`.

- [ ] **Step 3: Write the failing mock tests**

Append to `polyoxide-relay/tests/mock_api.rs` (it already has `client_unauthed`, `client_with_builder_auth`; add `use mockito::Matcher;` if not imported):

```rust
// ── v1 account routes ──────────────────────────────────────────

#[tokio::test]
async fn get_execute_params_asks_for_the_wallet_type_nonce() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into()),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"12"}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let nonce = client
        .get_execute_params(owner, polyoxide_relay::WalletType::DepositWallet)
        .await
        .unwrap();
    assert_eq!(nonce, 12);
    mock.assert_async().await;
}

#[tokio::test]
async fn get_deployed_typed_passes_the_type() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".into()),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let wallet: alloy::primitives::Address = "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse().unwrap();
    assert!(client
        .get_deployed_typed(wallet, polyoxide_relay::WalletType::DepositWallet)
        .await
        .unwrap());
    mock.assert_async().await;
}

#[tokio::test]
async fn get_gasless_transaction_reads_the_v1_record() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/account/transactions/tx-77")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transaction_id":"tx-77","transaction_hash":"0xabc","state":"STATE_CONFIRMED","error_msg":null}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let tx = client.get_gasless_transaction("tx-77").await.unwrap();
    assert_eq!(tx.state, polyoxide_relay::TransactionState::Confirmed);
    assert_eq!(tx.transaction_hash.as_deref(), Some("0xabc"));
    mock.assert_async().await;
}

#[tokio::test]
async fn resolve_wallet_probes_both_deposit_wallet_generations_and_the_safe() {
    // Anvil #0: beacon 0xBc0f…, uups 0xdf8b…, safe 0xd93B… (relay_vectors.json).
    let mut server = Server::new_async().await;
    let beacon = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".into()),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .create_async()
        .await;
    let uups = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xdf8b9E8f9AB23f261F6e1B171B7454ae6E46Ba76".into()),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .create_async()
        .await;
    let safe = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xd93B25cb943D14d0d34FBaF01Fc93a0f8b5F6E47".into()),
            Matcher::UrlEncoded("type".into(), "SAFE".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let kind = client.resolve_wallet(owner).await.unwrap();
    assert_eq!(
        kind,
        polyoxide_relay::WalletKind::DepositWallet(
            "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse().unwrap()
        )
    );
    beacon.assert_async().await;
    uups.assert_async().await;
    safe.assert_async().await;
}

#[tokio::test]
async fn resolve_wallet_refuses_two_deployed_wallets() {
    let mut server = Server::new_async().await;
    let _all_deployed = server
        .mock("GET", "/deployed")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .expect(3)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let err = client.resolve_wallet(owner).await.unwrap_err().to_string();
    assert!(err.contains("more than one"), "{err}");
}

#[tokio::test]
async fn resolve_wallet_reports_none_when_nothing_is_deployed() {
    let mut server = Server::new_async().await;
    let _none = server
        .mock("GET", "/deployed")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .expect(3)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    assert_eq!(client.resolve_wallet(owner).await.unwrap(), polyoxide_relay::WalletKind::None);
}
```

Run: `cargo test -p polyoxide-relay --test mock_api` — expect compile errors.

- [ ] **Step 4: Implement the client routes**

In `polyoxide-relay/src/client.rs`, add to the `use crate::types::{...}` import `ExecuteParams, GaslessTransaction`, and add to `impl RelayClient` after `get_deployed`:

```rust
    /// Check whether a wallet of the given type is deployed (`GET /deployed?type=`).
    ///
    /// The relayer answers for [`WalletType::Safe`] and [`WalletType::DepositWallet`];
    /// a Proxy auto-deploys on first use and is not queryable here.
    pub async fn get_deployed_typed(
        &self,
        wallet: Address,
        wallet_type: WalletType,
    ) -> Result<bool, RelayError> {
        #[derive(serde::Deserialize)]
        struct DeployedResponse {
            deployed: bool,
        }
        let url = self.http_client.base_url.join(&format!(
            "deployed?address={}&type={}",
            wallet,
            wallet_type.as_str()
        ))?;
        let resp = self.get_with_retry("/deployed", &url).await?;
        Ok(resp.json::<DeployedResponse>().await?.deployed)
    }

    /// Fetch the next nonce for `owner`'s wallet of `wallet_type`
    /// (`GET /v1/account/transactions/params`).
    ///
    /// This is the v1 route the Deposit Wallet batch needs; [`RelayClient::get_nonce`]
    /// stays on the legacy `/nonce` route for Safe and Proxy.
    pub async fn get_execute_params(
        &self,
        owner: Address,
        wallet_type: WalletType,
    ) -> Result<u64, RelayError> {
        let url = self.http_client.base_url.join(&format!(
            "v1/account/transactions/params?address={}&type={}",
            owner,
            wallet_type.as_str()
        ))?;
        let resp = self
            .get_with_retry("/v1/account/transactions/params", &url)
            .await?;
        Ok(resp.json::<ExecuteParams>().await?.nonce)
    }

    /// Poll a submitted transaction (`GET /v1/account/transactions/{id}`).
    ///
    /// Stop polling once [`TransactionState::is_terminal`] is true. The venue may take
    /// up to five minutes to move a session-signer authorization out of `STATE_NEW`.
    pub async fn get_gasless_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<GaslessTransaction, RelayError> {
        let url = self
            .http_client
            .base_url
            .join(&format!("v1/account/transactions/{}", transaction_id))?;
        let resp = self
            .get_with_retry("/v1/account/transactions", &url)
            .await?;
        resp.json::<GaslessTransaction>().await.map_err(Into::into)
    }

    /// Find which account wallet `owner` has deployed.
    ///
    /// Derives the beacon and UUPS Deposit Wallets and the Safe, asks `/deployed`
    /// for each, and returns the one that exists. More than one deployed wallet is
    /// an error rather than a guess. A Proxy wallet cannot be observed this way;
    /// see [`WalletKind`].
    pub async fn resolve_wallet(&self, owner: Address) -> Result<WalletKind, RelayError> {
        let cfg = &self.contract_config;
        let mut candidates: Vec<(WalletKind, WalletType)> = Vec::new();
        if cfg.deposit_wallet_factory.is_some() {
            candidates.push((
                WalletKind::DepositWallet(crate::wallet::derive_deposit_wallet_beacon(owner, cfg)?),
                WalletType::DepositWallet,
            ));
            candidates.push((
                WalletKind::DepositWallet(crate::wallet::derive_deposit_wallet_uups(owner, cfg)?),
                WalletType::DepositWallet,
            ));
        }
        candidates.push((WalletKind::Safe(crate::wallet::derive_safe(owner, cfg)), WalletType::Safe));

        let mut found = Vec::new();
        for (kind, wallet_type) in candidates {
            let address = kind.address().expect("candidate kinds carry an address");
            if self.get_deployed_typed(address, wallet_type).await? {
                found.push(kind);
            }
        }
        match found.as_slice() {
            [] => Ok(WalletKind::None),
            [one] => Ok(*one),
            many => Err(RelayError::Api(format!(
                "owner {owner} has more than one deployed wallet: {many:?}"
            ))),
        }
    }
```

Add `use crate::wallet::WalletKind;` to client.rs and `TransactionState` to the types import if the doc link needs it. Rate limiting needs no change: `RateLimiter::relay_default()` in `polyoxide-core/src/rate_limit.rs` has an empty per-path table and one default bucket (25 per 60 s), so the v1 routes draw on that bucket like every other relay path.

In `polyoxide-relay/src/lib.rs`, extend the `pub use types::{...}` list with `ExecuteParams, GaslessTransaction, TransactionState`.

- [ ] **Step 5: Verify**

Run: `cargo test -p polyoxide-relay` (unit + mock) — all pass. Gates.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-relay/src/types.rs polyoxide-relay/src/client.rs polyoxide-relay/src/lib.rs polyoxide-relay/tests/mock_api.rs
git commit -m "feat(relay): WalletType::DepositWallet, v1 params and transaction routes, resolve_wallet"
```

---

### Task 5: The Deposit Wallet `Batch`, pinned to py-sdk

**Files:**
- Create: `polyoxide-relay/src/deposit_wallet.rs`
- Modify: `polyoxide-relay/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-relay/src/deposit_wallet.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, Address, B256};
    use alloy::signers::local::PrivateKeySigner;
    use alloy::signers::Signer as _;

    const VECTORS: &str = include_str!("../tests/fixtures/session_keys/relay_vectors.json");
    const ANVIL_KEY_0: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn vectors() -> serde_json::Value {
        serde_json::from_str(VECTORS).unwrap()
    }

    fn wallet() -> Address {
        vectors()["wallet"].as_str().unwrap().parse().unwrap()
    }

    fn batch_from(v: &serde_json::Value) -> (Vec<DepositWalletCall>, u64, u64) {
        let calls = v["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| DepositWalletCall {
                target: c["target"].as_str().unwrap().parse().unwrap(),
                value: c["value"].as_str().unwrap().parse().unwrap(),
                data: hex::decode(c["data"].as_str().unwrap().trim_start_matches("0x")).unwrap().into(),
            })
            .collect();
        let nonce = v["nonce"].as_str().unwrap().parse().unwrap();
        let deadline = v["deadline"].as_str().unwrap().parse().unwrap();
        (calls, nonce, deadline)
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        hex::decode(s.trim_start_matches("0x")).unwrap()
    }

    #[test]
    fn batch_type_string_is_the_venues() {
        use alloy::sol_types::SolStruct;
        assert_eq!(
            protocol::Batch::eip712_encode_type(),
            "Batch(address wallet,uint256 nonce,uint256 deadline,Call[] calls)Call(address target,uint256 value,bytes data)"
        );
    }

    #[test]
    fn batch_digest_matches_py_sdk_for_all_four_batches() {
        let v = vectors();
        // multi_batch has two calls, the second with value 1, so Call[] hashing and
        // the value field are exercised, not just single zero-value calls.
        for name in ["approval_batch", "authorize_batch", "revoke_batch", "redeem_batch", "multi_batch"] {
            let (calls, nonce, deadline) = batch_from(&v[name]);
            let expected: B256 = v[name]["digest"].as_str().unwrap().parse().unwrap();
            assert_eq!(batch_digest(137, wallet(), &calls, nonce, deadline), expected, "{name}");
        }
    }

    #[tokio::test]
    async fn signing_the_digest_reproduces_py_sdk_signatures() {
        let v = vectors();
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        for name in ["approval_batch", "authorize_batch", "revoke_batch", "redeem_batch", "multi_batch"] {
            let (calls, nonce, deadline) = batch_from(&v[name]);
            let digest = batch_digest(137, wallet(), &calls, nonce, deadline);
            let sig = signer.sign_hash(&digest).await.unwrap();
            assert_eq!(
                format!("0x{}", hex::encode(sig.as_bytes())),
                v[name]["signature"].as_str().unwrap(),
                "{name}"
            );
        }
    }

    #[test]
    fn typed_data_json_equals_py_sdk_for_the_authorize_and_multi_batches() {
        let v = vectors();
        for name in ["authorize_batch", "multi_batch"] {
            let (calls, nonce, deadline) = batch_from(&v[name]);
            let json = batch_typed_data(137, wallet(), &calls, nonce, deadline);
            assert_eq!(json, v[name]["typed_data"], "{name}");
        }
    }

    #[test]
    fn session_envelope_reproduces_py_sdk_bytes() {
        let v = vectors();
        let session: Address = v["session_signer"].as_str().unwrap().parse().unwrap();
        let wrapped = wrap_session_signer(session, &hex_bytes(v["approval_batch"]["signature"].as_str().unwrap()));
        assert_eq!(wrapped.len(), 256);
        assert_eq!(wrapped, hex_bytes(v["approval_batch"]["session_signature"].as_str().unwrap()));
    }

    #[test]
    fn calldata_encoders_match_py_sdk() {
        let v = vectors();
        let session: Address = v["session_signer"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            format!("0x{}", hex::encode(authorize_session_signer_calldata(session, 1815534000))),
            v["authorize_batch"]["calls"][0]["data"].as_str().unwrap()
        );
        assert_eq!(
            format!("0x{}", hex::encode(revoke_session_signer_calldata(session))),
            v["revoke_batch"]["calls"][0]["data"].as_str().unwrap()
        );
        assert_eq!(
            format!("0x{}", hex::encode(erc20_approve_calldata(
                address!("E111180000d2663C0091e4f400237545B87B996B"),
                alloy::primitives::U256::MAX
            ))),
            v["approval_batch"]["calls"][0]["data"].as_str().unwrap()
        );
        let condition: B256 = "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51".parse().unwrap();
        assert_eq!(
            format!("0x{}", hex::encode(redeem_positions_calldata(
                address!("C011a7E12a19f7B1f670d46F03B03f3342E82DFB"),
                condition,
                &[alloy::primitives::U256::from(1), alloy::primitives::U256::from(2)]
            ))),
            v["redeem_batch"]["calls"][0]["data"].as_str().unwrap()
        );
    }

    #[test]
    fn session_lifetime_is_the_venues_fixed_value() {
        assert_eq!(SESSION_KEY_LIFETIME_SECS, 4_315 * 60 * 60);
        assert_eq!(DEFAULT_BATCH_DEADLINE_SECS, 600);
    }
}
```

Add `pub mod deposit_wallet;` to `polyoxide-relay/src/lib.rs`.

Run: `cargo test -p polyoxide-relay deposit_wallet::` — expect compile errors.

- [ ] **Step 2: Implement**

Prepend to `polyoxide-relay/src/deposit_wallet.rs`:

```rust
//! The Deposit Wallet `Batch`: what an owner (or session key) signs so the
//! relayer can execute calls through the wallet.
//!
//! The wallet verifies a plain EIP-712 `Batch` under its own domain
//! (`DepositWallet` v1, verifying contract = the wallet). There is no ERC-7739
//! layer here, unlike CLOB orders. A session key's signature is additionally
//! wrapped in the ERC-6492-style session-signer envelope, exactly as for orders;
//! the relay keeps its own copy of that wrapper because it does not depend on
//! `polyoxide-clob`, and pins it to the same py-sdk bytes.
//!
//! Everything here is pure and pinned to `tests/fixtures/session_keys/relay_vectors.json`.

use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::sol;
use alloy::sol_types::{Eip712Domain, SolCall, SolStruct, SolValue};

/// The venue's fixed session-key lifetime: 4 315 hours (a few hours under 180 days).
/// Other values are rejected by the relayer.
pub const SESSION_KEY_LIFETIME_SECS: u64 = 4_315 * 60 * 60;

/// Default `deadline` horizon for a batch, as the official SDKs use. The relayer
/// requires at least 10 s of validity on receipt.
pub const DEFAULT_BATCH_DEADLINE_SECS: u64 = 600;

/// The 32-byte suffix that marks a session-signer envelope (`0x6492` × 16).
pub const SESSION_SIGNER_MAGIC: [u8; 32] =
    alloy::primitives::hex!("6492649264926492649264926492649264926492649264926492649264926492");

/// One call inside a Deposit Wallet batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositWalletCall {
    /// Contract to call.
    pub target: Address,
    /// Native value to send (wei).
    pub value: U256,
    /// ABI-encoded calldata.
    pub data: Bytes,
}

pub(crate) mod protocol {
    use super::*;
    sol! {
        #[derive(Debug, PartialEq, Eq)]
        struct Call {
            address target;
            uint256 value;
            bytes data;
        }

        #[derive(Debug, PartialEq, Eq)]
        struct Batch {
            address wallet;
            uint256 nonce;
            uint256 deadline;
            Call[] calls;
        }

        function authorizeSessionSigner(address sessionSigner, uint256 validUntil);
        function revokeSessionSigner(address sessionSigner);
        function approve(address spender, uint256 amount);
        function setApprovalForAll(address operator, bool approved);
        function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);
    }
}

/// The wallet's EIP-712 domain: `DepositWallet` v1 with the wallet as verifying contract.
pub fn domain(chain_id: u64, wallet: Address) -> Eip712Domain {
    Eip712Domain {
        name: Some("DepositWallet".into()),
        version: Some("1".into()),
        chain_id: Some(U256::from(chain_id)),
        verifying_contract: Some(wallet),
        salt: None,
    }
}

fn to_protocol(wallet: Address, calls: &[DepositWalletCall], nonce: u64, deadline: u64) -> protocol::Batch {
    protocol::Batch {
        wallet,
        nonce: U256::from(nonce),
        deadline: U256::from(deadline),
        calls: calls
            .iter()
            .map(|c| protocol::Call {
                target: c.target,
                value: c.value,
                data: c.data.clone(),
            })
            .collect(),
    }
}

/// The digest the owner (or session key) signs for a batch.
pub fn batch_digest(chain_id: u64, wallet: Address, calls: &[DepositWalletCall], nonce: u64, deadline: u64) -> B256 {
    to_protocol(wallet, calls, nonce, deadline).eip712_signing_hash(&domain(chain_id, wallet))
}

/// The batch as EIP-712 JSON for `eth_signTypedData_v4`, byte-equal in field
/// names and order to what Polymarket's SDKs emit.
pub fn batch_typed_data(chain_id: u64, wallet: Address, calls: &[DepositWalletCall], nonce: u64, deadline: u64) -> serde_json::Value {
    serde_json::json!({
        "domain": {
            "chainId": chain_id,
            "name": "DepositWallet",
            "verifyingContract": wallet.to_string(),
            "version": "1"
        },
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" }
            ],
            "Batch": [
                { "name": "wallet", "type": "address" },
                { "name": "nonce", "type": "uint256" },
                { "name": "deadline", "type": "uint256" },
                { "name": "calls", "type": "Call[]" }
            ],
            "Call": [
                { "name": "target", "type": "address" },
                { "name": "value", "type": "uint256" },
                { "name": "data", "type": "bytes" }
            ]
        },
        "primaryType": "Batch",
        "message": {
            "wallet": wallet.to_string(),
            "nonce": nonce,
            "deadline": deadline,
            "calls": calls.iter().map(|c| serde_json::json!({
                "target": c.target.to_string(),
                "value": c.value.to::<u64>(),
                "data": format!("0x{}", hex::encode(&c.data)),
            })).collect::<Vec<_>>()
        }
    })
}

/// Wrap a 65-byte signature in the session-signer envelope.
///
/// `abi.encode(bytes32(leftPad(session_signer)), bytes32(0), bytes(signature)) ‖ 0x6492…6492`.
pub fn wrap_session_signer(session_signer: Address, signature: &[u8]) -> Vec<u8> {
    let signer_id = B256::left_padding_from(session_signer.as_slice());
    let mut out = (signer_id, B256::ZERO, Bytes::copy_from_slice(signature)).abi_encode_params();
    out.extend_from_slice(&SESSION_SIGNER_MAGIC);
    out
}

/// Calldata for `authorizeSessionSigner(address,uint256)` on the wallet.
pub fn authorize_session_signer_calldata(session_signer: Address, valid_until: u64) -> Vec<u8> {
    protocol::authorizeSessionSignerCall {
        sessionSigner: session_signer,
        validUntil: U256::from(valid_until),
    }
    .abi_encode()
}

/// Calldata for `revokeSessionSigner(address)` on the wallet.
pub fn revoke_session_signer_calldata(session_signer: Address) -> Vec<u8> {
    protocol::revokeSessionSignerCall { sessionSigner: session_signer }.abi_encode()
}

/// Calldata for ERC-20 `approve(spender, amount)`.
pub fn erc20_approve_calldata(spender: Address, amount: U256) -> Vec<u8> {
    protocol::approveCall { spender, amount }.abi_encode()
}

/// Calldata for ERC-1155 `setApprovalForAll(operator, approved)`.
pub fn erc1155_set_approval_for_all_calldata(operator: Address, approved: bool) -> Vec<u8> {
    protocol::setApprovalForAllCall { operator, approved }.abi_encode()
}

/// Calldata for `redeemPositions(collateral, 0x0, conditionId, indexSets)` on the CTF.
pub fn redeem_positions_calldata(collateral: Address, condition_id: B256, index_sets: &[U256]) -> Vec<u8> {
    protocol::redeemPositionsCall {
        collateral,
        parentCollectionId: B256::ZERO,
        conditionId: condition_id,
        indexSets: index_sets.to_vec(),
    }
    .abi_encode()
}
```

`value` in the typed data: py-sdk emits it as a JSON integer (the `multi_batch` fixture has a call with value 1). Emit it as a number when it fits in `u64`; for a larger value emit `serde_json::Number` from its decimal string via `serde_json::from_str::<serde_json::Value>(&c.value.to_string())`, which keeps it a JSON number as py-sdk would, rather than a string. The `hex` crate is a dependency.

In `polyoxide-relay/src/lib.rs`, add `pub use deposit_wallet::{DepositWalletCall, DEFAULT_BATCH_DEADLINE_SECS, SESSION_KEY_LIFETIME_SECS};` (the functions stay reachable as `polyoxide_relay::deposit_wallet::*`).

- [ ] **Step 3: Verify**

Run: `cargo test -p polyoxide-relay deposit_wallet::` — 7 passed. If `batch_digest_matches_py_sdk_for_all_four_batches` fails, the struct layout or domain is wrong; never touch the vector. Gates.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-relay/src/deposit_wallet.rs polyoxide-relay/src/lib.rs
git commit -m "feat(relay): Deposit Wallet Batch typed data, calldata encoders and session envelope pinned to py-sdk"
```

---

### Task 6: Auth without a key, and `execute` for a Deposit Wallet

**Files:**
- Modify: `polyoxide-relay/src/account.rs` (`BuilderAccount::with_signer`, type-erased signer)
- Modify: `polyoxide-relay/src/client.rs` (fields, builder, auth resolution, `post_json`, `submit_deposit_wallet_batch`, `deposit_wallet_batch_typed_data`, `execute` arm, approvals helper)
- Modify: `polyoxide-relay/src/lib.rs`
- Modify: `polyoxide-relay/tests/mock_api.rs`

- [ ] **Step 1: Write the failing mock tests**

Append to `polyoxide-relay/tests/mock_api.rs`:

```rust
// ── Deposit Wallet execution ───────────────────────────────────

const RELAY_VECTORS: &str = include_str!("fixtures/session_keys/relay_vectors.json");

fn relay_vectors() -> serde_json::Value {
    serde_json::from_str(RELAY_VECTORS).unwrap()
}

fn deposit_wallet_client(server: &mockito::ServerGuard) -> RelayClient {
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let wallet: alloy::primitives::Address = relay_vectors()["wallet"].as_str().unwrap().parse().unwrap();
    RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .build()
        .expect("build client")
}

#[tokio::test]
async fn submit_deposit_wallet_batch_posts_the_py_sdk_body_under_builder_hmac() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("POLY_BUILDER_SIGNATURE", Matcher::Any)
        .match_header("content-type", "application/json")
        .match_body(Matcher::Json(v["submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-1","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let b = &v["approval_batch"];
    let call = polyoxide_relay::DepositWalletCall {
        target: b["calls"][0]["target"].as_str().unwrap().parse().unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(b["calls"][0]["data"].as_str().unwrap()).unwrap().into(),
    };
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let resp = client
        .submit_deposit_wallet_batch(
            wallet,
            &[call],
            3,
            1_800_000_000,
            b["signature"].as_str().unwrap(),
            Some(String::new()),
        )
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-1");
    mock.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_fetches_the_wallet_nonce_and_signs_the_batch() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into()),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"3"}"#)
        .create_async()
        .await;
    // The deadline is now + 600 s, so the signature cannot be pinned here; the
    // batch_digest/signature vector tests pin the signing. Here: body shape and auth.
    let submit = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(
                r#"{{"type":"WALLET","from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","to":"{}","nonce":"3","depositWalletParams":{{"depositWallet":"{}","calls":[{{"target":"0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB","value":"0"}}]}}}}"#,
                v["config"]["deposit_wallet_factory"].as_str().unwrap(),
                v["wallet"].as_str().unwrap()
            )),
            Matcher::Regex(r#""signature":"0x[0-9a-f]{130}""#.into()),
            Matcher::Regex(r#""deadline":"\d{10}""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-2","state":"STATE_NEW"}"#)
        .create_async()
        .await;
    let legacy_nonce = server.mock("GET", "/nonce").match_query(Matcher::Any).expect(0).create_async().await;

    let client = deposit_wallet_client(&server);
    let tx = polyoxide_relay::SafeTransaction {
        to: "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB".parse().unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(v["approval_batch"]["calls"][0]["data"].as_str().unwrap()).unwrap().into(),
        operation: 0,
    };
    let resp = client.execute(vec![tx], Some(String::new())).await.unwrap();
    assert_eq!(resp.transaction_id, "tx-2");
    params.assert_async().await;
    submit.assert_async().await;
    legacy_nonce.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_refuses_delegatecall_before_io() {
    let mut server = Server::new_async().await;
    let params = server.mock("GET", "/v1/account/transactions/params").match_query(Matcher::Any).expect(0).create_async().await;
    let client = deposit_wallet_client(&server);
    let tx = polyoxide_relay::SafeTransaction {
        to: alloy::primitives::Address::ZERO,
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::Bytes::new(),
        operation: 1,
    };
    let err = client.execute(vec![tx], None).await.unwrap_err().to_string();
    assert!(err.contains("DELEGATECALL"), "{err}");
    params.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_needs_the_wallet_address() {
    let server = Server::new_async().await;
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .build()
        .unwrap();
    let tx = polyoxide_relay::SafeTransaction {
        to: alloy::primitives::Address::ZERO,
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::Bytes::new(),
        operation: 0,
    };
    let err = client.execute(vec![tx], None).await.unwrap_err().to_string();
    assert!(err.contains("deposit_wallet"), "{err}");
}

#[tokio::test]
async fn with_auth_submits_a_signature_in_batch_without_any_key() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::Json(v["submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-3","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let auth = polyoxide_relay::AuthConfig::Builder(BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into())));
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_auth(auth)
        .build()
        .unwrap();
    assert!(client.address().is_none());

    let b = &v["approval_batch"];
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let owner: alloy::primitives::Address = v["owner"].as_str().unwrap().parse().unwrap();
    let call = polyoxide_relay::DepositWalletCall {
        target: b["calls"][0]["target"].as_str().unwrap().parse().unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(b["calls"][0]["data"].as_str().unwrap()).unwrap().into(),
    };
    let typed = client.deposit_wallet_batch_typed_data(wallet, &[call.clone()], 3, 1_800_000_000);
    assert_eq!(typed, b["typed_data"]);
    let resp = client
        .submit_deposit_wallet_batch_from(owner, wallet, &[call], 3, 1_800_000_000, b["signature"].as_str().unwrap(), Some(String::new()))
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-3");
    mock.assert_async().await;
}
```

`Matcher::Json` compares parsed `serde_json::Value`s, so key order is irrelevant but address casing and value types must match the fixture exactly: the body must serialise addresses checksummed (`Address::to_string()`), `nonce`/`deadline`/`value` as decimal strings, and `metadata` as `""` when `Some(String::new())` is passed (omit the key when `None`).

Run: `cargo test -p polyoxide-relay --test mock_api` — expect compile errors.

- [ ] **Step 2: Type-erase `BuilderAccount`'s signer**

In `polyoxide-relay/src/account.rs`:

```rust
use std::sync::Arc;
use alloy::signers::Signer as AlloySigner;

/// Any `alloy` signer that implements `sign_hash`, type-erased. Local keys and
/// KMS-backed signers qualify; Ledger and Trezor refuse raw-hash signing and are
/// not usable here (hand them the typed data instead).
pub type DynSigner = dyn AlloySigner + Send + Sync;
```

Change the struct to `pub(crate) signer: Arc<DynSigner>, pub(crate) address: Address, pub(crate) config: Option<AuthConfig>`; `parse_signer` still parses a `PrivateKeySigner`, and each constructor stores `address: signer.address()` and `signer: Arc::new(signer)`. Add:

```rust
    /// Create an account around any `alloy` signer that supports `sign_hash`.
    pub fn with_signer<S>(signer: S, config: Option<AuthConfig>) -> Self
    where
        S: AlloySigner + Send + Sync + 'static,
    {
        Self {
            address: signer.address(),
            signer: Arc::new(signer),
            config,
        }
    }
```

`address()` returns the cached field; `signer()` returns `&DynSigner`. The existing `sign_message` call sites in `client.rs` (`execute_safe`, `execute_proxy`) keep compiling because `sign_message` is a trait method. Update the `Debug` impl (address only, as before). Export `DynSigner` from `lib.rs`.

- [ ] **Step 3: Auth without a key, and the post helper**

In `polyoxide-relay/src/client.rs`:

`RelayClient` gains `auth: Option<AuthConfig>`, `deposit_wallet: Option<Address>`, `deposit_wallet_role: DepositWalletRole` (import `polyoxide_core::DepositWalletRole`). `RelayClientBuilder` gains the same three (`auth: None`, `deposit_wallet: None`, `deposit_wallet_role: DepositWalletRole::Owner`) and setters:

```rust
    /// Authenticate relay submissions without a wallet key.
    ///
    /// For flows where the owner signs typed data out of process and this client
    /// only submits (session-signer authorization under Builder HMAC, or any
    /// `*_with_signature` call). An account's own auth config, if also set, takes
    /// precedence over this.
    pub fn with_auth(mut self, auth: AuthConfig) -> Self {
        self.auth = Some(auth);
        self
    }

    /// The Deposit Wallet this client acts for. Required by every Deposit Wallet
    /// execution path; it is not derived, so a mistaken address fails at the relayer
    /// rather than silently targeting a wallet you do not own.
    pub fn deposit_wallet(mut self, wallet: Address) -> Self {
        self.deposit_wallet = Some(wallet);
        self
    }

    /// Whether the account's key is the Deposit Wallet's owner (default) or a session
    /// key; a session key's batch signatures get the session-signer envelope.
    pub fn deposit_wallet_role(mut self, role: DepositWalletRole) -> Self {
        self.deposit_wallet_role = role;
        self
    }
```

In `build()`: `let auth = self.account.as_ref().and_then(|a| a.auth_config().cloned()).or(self.auth);` and store it. Replace the auth lookups in `authed_get_headers` and `_post_request` with a shared private `fn auth(&self) -> Result<&AuthConfig, RelayError>` that returns the existing "Account missing - cannot authenticate request…" message when both `account` and `auth` are `None` (the mock test `list_transactions_errors_when_no_auth_configured` asserts on it) and "No authentication configured…" when an account exists without auth.

Generalise `_post_request` into

```rust
    async fn post_json<B: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
        extra_headers: reqwest::header::HeaderMap,
        allow_relayer_api_key: bool,
    ) -> Result<T, RelayError>
```
with the same retry loop; `extra_headers` are merged after the auth headers each attempt; when `allow_relayer_api_key` is false and the auth is `AuthConfig::RelayerApiKey`, return `RelayError::Api(format!("{path} requires Builder HMAC auth; configure the client with BuilderConfig"))` before any I/O. Keep `_post_request` as `self.post_json(endpoint, body, HeaderMap::new(), true).await`.

- [ ] **Step 4: The Deposit Wallet execution path**

Add a private submit body next to the Safe and Proxy ones:

```rust
#[derive(Serialize)]
struct DepositWalletCallBody {
    target: String,
    value: String,
    data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DepositWalletParamsBody {
    deposit_wallet: String,
    deadline: String,
    calls: Vec<DepositWalletCallBody>,
}

#[derive(Serialize)]
struct DepositWalletSubmitBody {
    #[serde(rename = "type")]
    type_: String,
    from: String,
    to: String,
    nonce: String,
    signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
    #[serde(rename = "depositWalletParams")]
    deposit_wallet_params: DepositWalletParamsBody,
}
```

Add to `impl RelayClient`:

```rust
    fn deposit_wallet_factory(&self) -> Result<Address, RelayError> {
        self.contract_config.deposit_wallet_factory.ok_or_else(|| {
            RelayError::Api("Deposit Wallets are not supported on this chain".to_string())
        })
    }

    fn configured_deposit_wallet(&self) -> Result<Address, RelayError> {
        self.deposit_wallet.ok_or_else(|| {
            RelayError::Api(
                "no Deposit Wallet configured: call RelayClientBuilder::deposit_wallet(address)"
                    .to_string(),
            )
        })
    }

    /// Unix seconds now plus [`DEFAULT_BATCH_DEADLINE_SECS`].
    fn default_deadline() -> u64 {
        polyoxide_core::current_timestamp() + crate::deposit_wallet::DEFAULT_BATCH_DEADLINE_SECS
    }

    /// The batch as EIP-712 JSON for an external signer (`eth_signTypedData_v4`).
    ///
    /// Pair with [`RelayClient::submit_deposit_wallet_batch_from`]. `nonce` comes from
    /// [`RelayClient::get_execute_params`] for the owner with [`WalletType::DepositWallet`].
    pub fn deposit_wallet_batch_typed_data(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
    ) -> serde_json::Value {
        crate::deposit_wallet::batch_typed_data(self.chain_id, wallet, calls, nonce, deadline)
    }

    /// Submit a batch signed elsewhere, naming the signer explicitly.
    ///
    /// `from` is the EOA that produced `signature` (the owner, or a session key whose
    /// signature is already wrapped in the session-signer envelope). Works with
    /// [`RelayClientBuilder::with_auth`] and no account.
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_deposit_wallet_batch_from(
        &self,
        from: Address,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
        signature: &str,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let body = DepositWalletSubmitBody {
            type_: WalletType::DepositWallet.as_str().to_string(),
            from: from.to_string(),
            to: self.deposit_wallet_factory()?.to_string(),
            nonce: nonce.to_string(),
            signature: signature.to_string(),
            metadata,
            deposit_wallet_params: DepositWalletParamsBody {
                deposit_wallet: wallet.to_string(),
                deadline: deadline.to_string(),
                calls: calls
                    .iter()
                    .map(|c| DepositWalletCallBody {
                        target: c.target.to_string(),
                        value: c.value.to_string(),
                        data: format!("0x{}", hex::encode(&c.data)),
                    })
                    .collect(),
            },
        };
        self._post_request("submit", &body).await
    }

    /// Submit a batch signed elsewhere by this client's account.
    pub async fn submit_deposit_wallet_batch(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
        signature: &str,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let from = self.account.as_ref().ok_or(RelayError::MissingSigner)?.address();
        self.submit_deposit_wallet_batch_from(from, wallet, calls, nonce, deadline, signature, metadata)
            .await
    }

    /// Sign a batch with the account's key, applying the session-signer envelope for a
    /// session-key role, and return the hex signature.
    async fn sign_deposit_wallet_batch(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
    ) -> Result<String, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        let digest = crate::deposit_wallet::batch_digest(self.chain_id, wallet, calls, nonce, deadline);
        let sig = account
            .signer()
            .sign_hash(&digest)
            .await
            .map_err(|e| RelayError::Signer(e.to_string()))?;
        let bytes = match self.deposit_wallet_role {
            DepositWalletRole::Owner => sig.as_bytes().to_vec(),
            DepositWalletRole::SessionKey => {
                crate::deposit_wallet::wrap_session_signer(account.address(), &sig.as_bytes())
            }
        };
        Ok(format!("0x{}", hex::encode(bytes)))
    }

    async fn execute_deposit_wallet(
        &self,
        transactions: Vec<SafeTransaction>,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let wallet = self.configured_deposit_wallet()?;
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        let calls = transactions
            .into_iter()
            .map(|tx| {
                if tx.operation != CALL_OPERATION {
                    return Err(RelayError::Api(
                        "a Deposit Wallet batch supports CALL only, not DELEGATECALL".to_string(),
                    ));
                }
                Ok(DepositWalletCall { target: tx.to, value: tx.value, data: tx.data })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let nonce = self
            .get_execute_params(account.address(), WalletType::DepositWallet)
            .await?;
        let deadline = Self::default_deadline();
        let signature = self.sign_deposit_wallet_batch(wallet, &calls, nonce, deadline).await?;
        self.submit_deposit_wallet_batch(wallet, &calls, nonce, deadline, &signature, metadata)
            .await
    }
```

In `execute_with_gas`, add the arm `WalletType::DepositWallet => self.execute_deposit_wallet(transactions, metadata).await,` (the gas limit is not part of a Deposit Wallet submission; document that it is ignored for this type). In `estimate_redemption_gas`, the `proxy_wallet` match gains `WalletType::DepositWallet => self.configured_deposit_wallet()?`.

Add a helper for the four trading approvals from the deposit-wallets page:

```rust
    /// The four approvals a Deposit Wallet needs before it can trade: pUSD `approve`
    /// and Conditional Tokens `setApprovalForAll` for the standard and neg-risk V2
    /// exchanges. Sign and submit the result as one batch, or convert each call to a
    /// `SafeTransaction` with `operation: 0` for [`RelayClient::execute`].
    ///
    /// The addresses are Polygon mainnet's; any other chain is an error.
    pub fn deposit_wallet_trading_approvals(&self) -> Result<Vec<DepositWalletCall>, RelayError> {
        use crate::deposit_wallet::{erc1155_set_approval_for_all_calldata, erc20_approve_calldata};
        if self.chain_id != 137 {
            return Err(RelayError::Api(format!(
                "trading approvals are only known for Polygon mainnet (137), not chain {}",
                self.chain_id
            )));
        }
        let pusd = address!("C011a7E12a19f7B1f670d46F03B03f3342E82DFB");
        let ctf = address!("4D97DCd97eC945f40cF65F87097ACe5EA0476045");
        let exchanges = [
            address!("E111180000d2663C0091e4f400237545B87B996B"),
            address!("e2222d279d744050d28e00520010520000310F59"),
        ];
        let mut calls = Vec::with_capacity(4);
        for exchange in exchanges {
            calls.push(DepositWalletCall { target: pusd, value: U256::ZERO, data: erc20_approve_calldata(exchange, U256::MAX).into() });
            calls.push(DepositWalletCall { target: ctf, value: U256::ZERO, data: erc1155_set_approval_for_all_calldata(exchange, true).into() });
        }
        Ok(calls)
    }
```
(`address!` is `alloy::primitives::address!`.) Add a unit test in `client.rs` asserting the helper returns four calls on chain 137, the first targeting pUSD with calldata starting `0x095ea7b3`, the second targeting the CTF with calldata starting `0xa22cb465`, and that it errors on chain 80002.

Export `DepositWalletCall` was done in Task 5; export nothing new here beyond `DynSigner`.

- [ ] **Step 5: Verify**

Run: `cargo test -p polyoxide-relay` — all unit and mock tests pass, including the five new mock tests; the README doctests (`cargo test -p polyoxide-relay --doc`) still compile. Gates.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-relay/src/account.rs polyoxide-relay/src/client.rs polyoxide-relay/src/lib.rs polyoxide-relay/tests/mock_api.rs
git commit -m "feat(relay): execute through a Deposit Wallet; auth without a key; typed-data-out, signature-in batches"
```

---

### Task 7: Session-signer endpoints

**Files:**
- Create: `polyoxide-relay/src/session_signers.rs`
- Modify: `polyoxide-relay/src/client.rs`, `polyoxide-relay/src/lib.rs`
- Modify: `polyoxide-relay/tests/mock_api.rs`

- [ ] **Step 1: Write the failing unit tests**

Create `polyoxide-relay/src/session_signers.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::SessionSignerScope;

    #[test]
    fn validate_scopes_accepts_the_documented_shapes() {
        assert!(validate_scopes(&[SessionSignerScope::Clob]).is_ok());
        assert!(validate_scopes(&[SessionSignerScope::Clob, SessionSignerScope::CombosRfq]).is_ok());
        assert!(validate_scopes(&[SessionSignerScope::All]).is_ok());
    }

    #[test]
    fn validate_scopes_rejects_empty_duplicates_and_all_with_others() {
        assert!(validate_scopes(&[]).unwrap_err().to_string().contains("at least one"));
        assert!(validate_scopes(&[SessionSignerScope::Clob, SessionSignerScope::Clob])
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
        assert!(validate_scopes(&[SessionSignerScope::All, SessionSignerScope::Clob])
            .unwrap_err()
            .to_string()
            .contains("ALL"));
        // Other("ALL") is compared on the wire spelling, so it cannot slip past.
        assert!(validate_scopes(&[SessionSignerScope::Other("ALL".into()), SessionSignerScope::Clob]).is_err());
    }

    #[test]
    fn statuses_round_trip_and_flag_terminal_failures() {
        let s: SessionSignerAuthorizationStatus = serde_json::from_str("\"REGISTRY_PENDING\"").unwrap();
        assert_eq!(s, SessionSignerAuthorizationStatus::RegistryPending);
        assert!(!s.is_terminal_failure());
        for wire in ["FAILED", "SUPERSEDED", "REPAIR_REQUIRED"] {
            assert!(SessionSignerAuthorizationStatus::from_wire(wire).is_terminal_failure(), "{wire}");
        }
        assert!(!SessionSignerAuthorizationStatus::Other("NEW_THING".into()).is_terminal_failure());

        let r: SessionSignerRevocationStatus = serde_json::from_str("\"FENCED\"").unwrap();
        assert_eq!(r, SessionSignerRevocationStatus::Fenced);
        assert!(SessionSignerRevocationStatus::Failed.is_terminal_failure());
        assert!(!SessionSignerRevocationStatus::Confirmed.is_terminal_failure());
    }

    #[test]
    fn responses_parse_the_sdk_shapes() {
        let a: SessionSignerAuthorizationResponse = serde_json::from_str(
            r#"{"operationId":"op-1","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-1"}"#,
        )
        .unwrap();
        assert_eq!(a.status, SessionSignerAuthorizationStatus::Submitted);
        assert_eq!(a.transaction_hash, None);
        assert_eq!(a.transaction_id, "tx-1");
        assert_eq!(a.operation_id.as_deref(), Some("op-1"));

        let r: SessionSignerRevocationResponse = serde_json::from_str(
            r#"{"operationId":"op-2","status":"FENCED","fenced":true,"transactionId":"tx-2"}"#,
        )
        .unwrap();
        assert_eq!(r.status, SessionSignerRevocationStatus::Fenced);
        assert!(r.fenced);
    }

    #[test]
    fn authorization_request_serialises_the_venue_body() {
        let req = SessionSignerAuthorization {
            wallet_address: "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse().unwrap(),
            session_signer_address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse().unwrap(),
            scopes: vec![SessionSignerScope::Clob],
            valid_until: 1815534000,
            nonce: 4,
            deadline: 1800000600,
        };
        let body = req.body("0xsig");
        let expected: serde_json::Value = serde_json::from_str(
            r#"{"deadline":"1800000600","nonce":"4","scopes":["CLOB"],"sessionSignerAddress":"0x70997970C51812dc3A010C7d01b50e0d17dc79C8","signature":"0xsig","validUntil":"1815534000","walletAddress":"0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50"}"#,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&body).unwrap(), expected);
    }
}
```

Add `pub mod session_signers;` to `lib.rs`. Run: `cargo test -p polyoxide-relay session_signers::` — expect compile errors.

- [ ] **Step 2: Implement the types**

Prepend to `polyoxide-relay/src/session_signers.rs`:

```rust
//! Session-signer authorization and revocation through the relayer
//! (`POST /v1/session-signers/authorizations` and `/revocations`).
//!
//! Neither route is in the published relayer OpenAPI; the contract is
//! `docs.polymarket.com/trading/session-keys` and Polymarket's official SDKs.
//! Both take a Deposit Wallet `Batch` signed by the owner (see
//! [`crate::deposit_wallet`]) plus the session-signer fields, under Builder HMAC
//! auth only, with an `Idempotency-Key` header.

use alloy::primitives::Address;
use polyoxide_core::SessionSignerScope;
use serde::{Deserialize, Serialize};

use crate::error::RelayError;
use crate::types::open_string_enum;

/// Reject the scope lists the venue refuses: empty, duplicated, or `ALL` mixed
/// with anything else. Compares on the wire spelling, so `Other("ALL")` counts as `ALL`.
pub fn validate_scopes(scopes: &[SessionSignerScope]) -> Result<(), RelayError> {
    if scopes.is_empty() {
        return Err(RelayError::Api("session-signer scopes need at least one entry".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for scope in scopes {
        if !seen.insert(scope.as_str()) {
            return Err(RelayError::Api(format!("duplicate session-signer scope {scope}")));
        }
    }
    if scopes.iter().any(|s| s.as_str() == "ALL") && scopes.len() > 1 {
        return Err(RelayError::Api("scope ALL must be requested alone".into()));
    }
    Ok(())
}

open_string_enum! {
    /// Status of a session-signer authorization operation.
    SessionSignerAuthorizationStatus {
        /// Accepted; batch not yet broadcast.
        Submitted => "SUBMITTED",
        /// Broadcast; not yet in the session-signer registry.
        RegistryPending => "REGISTRY_PENDING",
        /// Live: the key appears in `GET /v1/user/session-signers`.
        Registered => "REGISTERED",
        /// Terminal failure.
        Failed => "FAILED",
        /// Terminal: a newer authorization for the same signer replaced this one.
        Superseded => "SUPERSEDED",
        /// Terminal: the venue needs manual intervention.
        RepairRequired => "REPAIR_REQUIRED",
    }
}

impl SessionSignerAuthorizationStatus {
    /// `Failed`, `Superseded` or `RepairRequired`.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::Superseded | Self::RepairRequired)
    }
}

open_string_enum! {
    /// Status of a session-signer revocation operation.
    SessionSignerRevocationStatus {
        /// Accepted.
        Pending => "PENDING",
        /// The key is fenced out of the registry; its open orders are being cancelled.
        Fenced => "FENCED",
        /// Open orders cancelled.
        Swept => "SWEPT",
        /// The on-chain revocation is broadcast.
        ChainSubmitted => "CHAIN_SUBMITTED",
        /// Confirmed on chain.
        Confirmed => "CONFIRMED",
        /// Terminal failure.
        Failed => "FAILED",
    }
}

impl SessionSignerRevocationStatus {
    /// Only `Failed`.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

/// Everything an authorization body needs except the owner's signature.
///
/// Produced by [`crate::RelayClient::authorize_session_signer_typed_data`] together
/// with the typed data to sign; pass it back with the signature to
/// [`crate::RelayClient::submit_session_signer_authorization`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSignerAuthorization {
    /// The Deposit Wallet.
    pub wallet_address: Address,
    /// The session key's EOA.
    pub session_signer_address: Address,
    /// Requested venues.
    pub scopes: Vec<SessionSignerScope>,
    /// Expiry, Unix seconds: now + [`crate::deposit_wallet::SESSION_KEY_LIFETIME_SECS`] at build time.
    pub valid_until: u64,
    /// The wallet nonce the batch was built with.
    pub nonce: u64,
    /// The batch deadline, Unix seconds.
    pub deadline: u64,
}

/// Wire body of `POST /v1/session-signers/authorizations`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerAuthorizationBody {
    pub deadline: String,
    pub nonce: String,
    pub scopes: Vec<SessionSignerScope>,
    pub session_signer_address: String,
    pub signature: String,
    pub valid_until: String,
    pub wallet_address: String,
}

impl SessionSignerAuthorization {
    /// The wire body with `signature` filled in.
    pub fn body(&self, signature: &str) -> SessionSignerAuthorizationBody {
        SessionSignerAuthorizationBody {
            deadline: self.deadline.to_string(),
            nonce: self.nonce.to_string(),
            scopes: self.scopes.clone(),
            session_signer_address: self.session_signer_address.to_string(),
            signature: signature.to_string(),
            valid_until: self.valid_until.to_string(),
            wallet_address: self.wallet_address.to_string(),
        }
    }
}

/// Response of `POST /v1/session-signers/authorizations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerAuthorizationResponse {
    /// Opaque operation id.
    #[serde(default)]
    pub operation_id: Option<String>,
    pub status: SessionSignerAuthorizationStatus,
    /// Present once the batch is broadcast.
    #[serde(default)]
    pub transaction_hash: Option<String>,
    /// Poll this with [`crate::RelayClient::get_gasless_transaction`].
    pub transaction_id: String,
}

/// Everything a revocation body needs except the owner's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSignerRevocation {
    pub wallet_address: Address,
    pub session_signer_address: Address,
    pub nonce: u64,
    pub deadline: u64,
}

/// Wire body of `POST /v1/session-signers/revocations`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerRevocationBody {
    pub deadline: String,
    pub nonce: String,
    pub session_signer_address: String,
    pub signature: String,
    pub wallet_address: String,
}

impl SessionSignerRevocation {
    /// The wire body with `signature` filled in.
    pub fn body(&self, signature: &str) -> SessionSignerRevocationBody {
        SessionSignerRevocationBody {
            deadline: self.deadline.to_string(),
            nonce: self.nonce.to_string(),
            session_signer_address: self.session_signer_address.to_string(),
            signature: signature.to_string(),
            wallet_address: self.wallet_address.to_string(),
        }
    }
}

/// Response of `POST /v1/session-signers/revocations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerRevocationResponse {
    #[serde(default)]
    pub operation_id: Option<String>,
    pub status: SessionSignerRevocationStatus,
    /// Whether the key was already fenced out of the registry when the relayer answered.
    pub fenced: bool,
    pub transaction_id: String,
}
```

The crate does not deny `missing_docs`, but give every `pub` field of the two body structs a one-line doc anyway (they mirror the venue's field names: `deadline`, `nonce`, `scopes`, `sessionSignerAddress`, `signature`, `validUntil`, `walletAddress`).

Run: `cargo test -p polyoxide-relay session_signers::` — 5 passed.

- [ ] **Step 3: Write the failing mock tests**

Append to `polyoxide-relay/tests/mock_api.rs`:

```rust
// ── session signers ────────────────────────────────────────────

#[tokio::test]
async fn authorize_session_signer_typed_data_and_submit_reproduce_the_py_sdk_body() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/authorizations")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("POLY_BUILDER_SIGNATURE", Matcher::Any)
        .match_header("POLY_BUILDER_PASSPHRASE", "pp")
        .match_header("Idempotency-Key", "idem-1")
        .match_body(Matcher::Json(v["authorization_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"operationId":"op-1","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-9"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address = v["session_signer"].as_str().unwrap().parse().unwrap();
    let (typed, request) = client
        .authorize_session_signer_typed_data_with_valid_until(
            wallet,
            session,
            vec![polyoxide_core::SessionSignerScope::Clob],
            1815534000,
            4,
            1800000600,
        )
        .unwrap();
    assert_eq!(typed, v["authorize_batch"]["typed_data"]);
    assert_eq!(request.valid_until, 1815534000);

    let resp = client
        .submit_session_signer_authorization(&request, v["authorize_batch"]["signature"].as_str().unwrap(), "idem-1")
        .await
        .unwrap();
    assert_eq!(resp.status, polyoxide_relay::SessionSignerAuthorizationStatus::Submitted);
    assert_eq!(resp.transaction_id, "tx-9");
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_typed_data_computes_valid_until_from_the_lifetime() {
    let v = relay_vectors();
    let server = Server::new_async().await;
    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address = v["session_signer"].as_str().unwrap().parse().unwrap();
    let before = polyoxide_core::current_timestamp();
    let (_, request) = client
        .authorize_session_signer_typed_data(wallet, session, vec![polyoxide_core::SessionSignerScope::All], 4, 1800000600)
        .unwrap();
    let after = polyoxide_core::current_timestamp();
    assert!(request.valid_until >= before + polyoxide_relay::SESSION_KEY_LIFETIME_SECS);
    assert!(request.valid_until <= after + polyoxide_relay::SESSION_KEY_LIFETIME_SECS);
}

#[tokio::test]
async fn session_signer_routes_refuse_relayer_api_key_auth_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/authorizations")
        .expect(0)
        .create_async()
        .await;
    let client = client_with_relayer_api_key_auth(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address = v["session_signer"].as_str().unwrap().parse().unwrap();
    let (_, request) = client
        .authorize_session_signer_typed_data_with_valid_until(wallet, session, vec![polyoxide_core::SessionSignerScope::Clob], 1815534000, 4, 1800000600)
        .unwrap();
    let err = client
        .submit_session_signer_authorization(&request, "0xsig", "idem")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Builder HMAC"), "{err}");
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_typed_data_rejects_bad_scopes_before_io() {
    let v = relay_vectors();
    let server = Server::new_async().await;
    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address = v["session_signer"].as_str().unwrap().parse().unwrap();
    let err = client
        .authorize_session_signer_typed_data(wallet, session, vec![polyoxide_core::SessionSignerScope::All, polyoxide_core::SessionSignerScope::Clob], 4, 1800000600)
        .unwrap_err()
        .to_string();
    assert!(err.contains("ALL"), "{err}");
}

#[tokio::test]
async fn revoke_session_signer_typed_data_and_submit_send_the_venue_body() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/revocations")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("Idempotency-Key", "idem-2")
        .match_body(Matcher::Json(v["revocation_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"operationId":"op-2","status":"FENCED","fenced":true,"transactionId":"tx-10"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address = v["session_signer"].as_str().unwrap().parse().unwrap();
    let (typed, request) = client.revoke_session_signer_typed_data(wallet, session, 5, 1800000600);
    assert_eq!(typed, v["revoke_batch"]["typed_data"]);
    let resp = client
        .submit_session_signer_revocation(&request, v["revoke_batch"]["signature"].as_str().unwrap(), "idem-2")
        .await
        .unwrap();
    assert_eq!(resp.status, polyoxide_relay::SessionSignerRevocationStatus::Fenced);
    assert!(resp.fenced);
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_with_a_local_key_fetches_the_nonce_and_signs() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into()),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"4"}"#)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/v1/session-signers/authorizations")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("Idempotency-Key", Matcher::Regex(r"^[0-9a-f-]{36}$".into()))
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(
                r#"{{"nonce":"4","scopes":["CLOB"],"sessionSignerAddress":"{}","walletAddress":"{}"}}"#,
                v["session_signer"].as_str().unwrap(),
                v["wallet"].as_str().unwrap()
            )),
            Matcher::Regex(r#""signature":"0x[0-9a-f]{130}""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"operationId":"op-3","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-11"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let session: alloy::primitives::Address = v["session_signer"].as_str().unwrap().parse().unwrap();
    let resp = client
        .authorize_session_signer(session, vec![polyoxide_core::SessionSignerScope::Clob])
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-11");
    params.assert_async().await;
    submit.assert_async().await;
}
```

Run: `cargo test -p polyoxide-relay --test mock_api session_signer` — expect compile errors.

- [ ] **Step 4: Implement the client calls**

In `polyoxide-relay/src/client.rs`, add imports for the session-signer types and `SessionSignerScope`, and to `impl RelayClient`:

```rust
    /// Build the authorization batch for an external signer, with `valid_until`
    /// computed as now + [`SESSION_KEY_LIFETIME_SECS`] (the only lifetime the venue
    /// accepts). Returns the typed data to sign and the request to submit with the
    /// signature. Validates `scopes` before doing anything else.
    pub fn authorize_session_signer_typed_data(
        &self,
        wallet: Address,
        session_signer: Address,
        scopes: Vec<SessionSignerScope>,
        nonce: u64,
        deadline: u64,
    ) -> Result<(serde_json::Value, SessionSignerAuthorization), RelayError> {
        let valid_until =
            polyoxide_core::current_timestamp() + crate::deposit_wallet::SESSION_KEY_LIFETIME_SECS;
        self.authorize_session_signer_typed_data_with_valid_until(
            wallet, session_signer, scopes, valid_until, nonce, deadline,
        )
    }

    /// [`RelayClient::authorize_session_signer_typed_data`] with an explicit
    /// `valid_until`. The venue rejects lifetimes other than
    /// [`SESSION_KEY_LIFETIME_SECS`] from now; this exists for tests and for the
    /// day the venue relaxes that.
    pub fn authorize_session_signer_typed_data_with_valid_until(
        &self,
        wallet: Address,
        session_signer: Address,
        scopes: Vec<SessionSignerScope>,
        valid_until: u64,
        nonce: u64,
        deadline: u64,
    ) -> Result<(serde_json::Value, SessionSignerAuthorization), RelayError> {
        crate::session_signers::validate_scopes(&scopes)?;
        if session_signer == Address::ZERO {
            return Err(RelayError::Api("session signer must not be the zero address".into()));
        }
        let calls = vec![DepositWalletCall {
            target: wallet,
            value: U256::ZERO,
            data: crate::deposit_wallet::authorize_session_signer_calldata(session_signer, valid_until).into(),
        }];
        let typed = self.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline);
        Ok((
            typed,
            SessionSignerAuthorization {
                wallet_address: wallet,
                session_signer_address: session_signer,
                scopes,
                valid_until,
                nonce,
                deadline,
            },
        ))
    }

    /// `POST /v1/session-signers/authorizations` with an owner signature produced
    /// elsewhere. Builder HMAC auth only; `idempotency_key` is sent as
    /// `Idempotency-Key` and should be reused when retrying the same request.
    pub async fn submit_session_signer_authorization(
        &self,
        request: &SessionSignerAuthorization,
        signature: &str,
        idempotency_key: &str,
    ) -> Result<SessionSignerAuthorizationResponse, RelayError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Idempotency-Key",
            reqwest::header::HeaderValue::from_str(idempotency_key)
                .map_err(|e| RelayError::Api(format!("invalid idempotency key: {e}")))?,
        );
        self.post_json("v1/session-signers/authorizations", &request.body(signature), headers, false)
            .await
    }

    /// Build the revocation batch for an external signer.
    pub fn revoke_session_signer_typed_data(
        &self,
        wallet: Address,
        session_signer: Address,
        nonce: u64,
        deadline: u64,
    ) -> (serde_json::Value, SessionSignerRevocation) {
        let calls = vec![DepositWalletCall {
            target: wallet,
            value: U256::ZERO,
            data: crate::deposit_wallet::revoke_session_signer_calldata(session_signer).into(),
        }];
        let typed = self.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline);
        (typed, SessionSignerRevocation { wallet_address: wallet, session_signer_address: session_signer, nonce, deadline })
    }

    /// `POST /v1/session-signers/revocations` with an owner signature produced elsewhere.
    ///
    /// The venue answers once the key is fenced out of the registry; the on-chain
    /// revocation and the cancel-all of its open orders follow asynchronously.
    pub async fn submit_session_signer_revocation(
        &self,
        request: &SessionSignerRevocation,
        signature: &str,
        idempotency_key: &str,
    ) -> Result<SessionSignerRevocationResponse, RelayError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Idempotency-Key",
            reqwest::header::HeaderValue::from_str(idempotency_key)
                .map_err(|e| RelayError::Api(format!("invalid idempotency key: {e}")))?,
        );
        self.post_json("v1/session-signers/revocations", &request.body(signature), headers, false)
            .await
    }

    /// Authorize `session_signer` with this client's account (the owner's key) and
    /// configured Deposit Wallet: fetch the nonce, sign, submit. The idempotency key
    /// is a fresh UUID; use the two-step API to retry with the same one.
    pub async fn authorize_session_signer(
        &self,
        session_signer: Address,
        scopes: Vec<SessionSignerScope>,
    ) -> Result<SessionSignerAuthorizationResponse, RelayError> {
        let wallet = self.configured_deposit_wallet()?;
        let owner = self.account.as_ref().ok_or(RelayError::MissingSigner)?.address();
        let nonce = self.get_execute_params(owner, WalletType::DepositWallet).await?;
        let deadline = Self::default_deadline();
        let (_, request) = self.authorize_session_signer_typed_data(wallet, session_signer, scopes, nonce, deadline)?;
        let calls = vec![DepositWalletCall {
            target: wallet,
            value: U256::ZERO,
            data: crate::deposit_wallet::authorize_session_signer_calldata(session_signer, request.valid_until).into(),
        }];
        let signature = self.sign_deposit_wallet_batch(wallet, &calls, nonce, deadline).await?;
        self.submit_session_signer_authorization(&request, &signature, &Self::new_idempotency_key())
            .await
    }

    /// Revoke `session_signer` with this client's account and configured Deposit Wallet.
    pub async fn revoke_session_signer(
        &self,
        session_signer: Address,
    ) -> Result<SessionSignerRevocationResponse, RelayError> {
        let wallet = self.configured_deposit_wallet()?;
        let owner = self.account.as_ref().ok_or(RelayError::MissingSigner)?.address();
        let nonce = self.get_execute_params(owner, WalletType::DepositWallet).await?;
        let deadline = Self::default_deadline();
        let (_, request) = self.revoke_session_signer_typed_data(wallet, session_signer, nonce, deadline);
        let calls = vec![DepositWalletCall {
            target: wallet,
            value: U256::ZERO,
            data: crate::deposit_wallet::revoke_session_signer_calldata(session_signer).into(),
        }];
        let signature = self.sign_deposit_wallet_batch(wallet, &calls, nonce, deadline).await?;
        self.submit_session_signer_revocation(&request, &signature, &Self::new_idempotency_key())
            .await
    }

    /// A UUID v4 string for `Idempotency-Key`.
    fn new_idempotency_key() -> String {
        // Avoid a uuid dependency: 16 random bytes formatted 8-4-4-4-12 with the
        // version and variant nibbles set.
        let mut b = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rng(), &mut b);
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        let h = hex::encode(b);
        format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
    }
```

`rand` is a dependency of `polyoxide-clob`, not relay; add `rand = { version = "0.9.2", features = ["std"] }` to `polyoxide-relay/Cargo.toml` `[dependencies]` (same version as clob). The `rand 0.9` API is `rand::rng()` and `RngCore::fill_bytes`. Also make sure `post_json` sets `Content-Type: application/json` (it does, inherited from `_post_request`) and that the HMAC covers the exact body string it sends.

The session-key role: `authorize_session_signer` / `revoke_session_signer` are owner operations; if `self.deposit_wallet_role` is `SessionKey`, return `RelayError::Api("session signers are managed by the wallet owner, not a session key")` before I/O in both conveniences.

Export from `lib.rs`: `pub use session_signers::{validate_scopes, SessionSignerAuthorization, SessionSignerAuthorizationBody, SessionSignerAuthorizationResponse, SessionSignerAuthorizationStatus, SessionSignerRevocation, SessionSignerRevocationBody, SessionSignerRevocationResponse, SessionSignerRevocationStatus};` and `pub use polyoxide_core::{DepositWalletRole, SessionSignerScope};`.

- [ ] **Step 5: Verify**

Run: `cargo test -p polyoxide-relay` — all pass, including the six new mock tests. Gates.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-relay/Cargo.toml Cargo.lock polyoxide-relay/src/session_signers.rs polyoxide-relay/src/client.rs polyoxide-relay/src/lib.rs polyoxide-relay/tests/mock_api.rs
git commit -m "feat(relay): session-signer authorization and revocation under Builder HMAC"
```

---

### Task 8: Redemption pair, docs, and gates

**Files:**
- Modify: `polyoxide-relay/src/client.rs` (`redeem_typed_data`, `submit_redemption_with_signature`, `submit_gasless_redemption` on a Deposit Wallet)
- Modify: `polyoxide-relay/src/lib.rs` (crate docs)
- Modify: `polyoxide-relay/README.md` (one section; it is a doctest)
- Modify: `polyoxide-relay/tests/mock_api.rs`

- [ ] **Step 1: Write the failing mock test**

Append to `polyoxide-relay/tests/mock_api.rs`:

```rust
#[tokio::test]
async fn redeem_typed_data_and_submit_reproduce_the_py_sdk_batch() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::Json(v["redeem_submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-12","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let condition: alloy::primitives::B256 = "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51".parse().unwrap();
    let index_sets = [alloy::primitives::U256::from(1), alloy::primitives::U256::from(2)];
    let (typed, calls) = client.redeem_typed_data(wallet, condition, &index_sets, 6, 1800000600);
    assert_eq!(typed, v["redeem_batch"]["typed_data"]);
    let resp = client
        .submit_redemption_with_signature(wallet, &calls, 6, 1800000600, v["redeem_batch"]["signature"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-12");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Implement**

In `client.rs`:

```rust
    /// The redemption batch for a Deposit Wallet, for an external signer.
    ///
    /// Redeems `condition_id`'s `index_sets` on the Conditional Tokens contract with
    /// pUSD as collateral (the Deposit Wallet collateral, not the legacy USDC). Returns
    /// the typed data and the calls to pass to
    /// [`RelayClient::submit_redemption_with_signature`].
    pub fn redeem_typed_data(
        &self,
        wallet: Address,
        condition_id: B256,
        index_sets: &[U256],
        nonce: u64,
        deadline: u64,
    ) -> (serde_json::Value, Vec<DepositWalletCall>) {
        let calls = vec![DepositWalletCall {
            target: address!("4D97DCd97eC945f40cF65F87097ACe5EA0476045"),
            value: U256::ZERO,
            data: crate::deposit_wallet::redeem_positions_calldata(
                address!("C011a7E12a19f7B1f670d46F03B03f3342E82DFB"),
                condition_id,
                index_sets,
            )
            .into(),
        }];
        (self.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline), calls)
    }

    /// Submit a redemption batch signed elsewhere by this client's account.
    ///
    /// Sends `metadata: ""`, as py-sdk does for every Deposit Wallet submission.
    pub async fn submit_redemption_with_signature(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
        signature: &str,
    ) -> Result<SubmitResponse, RelayError> {
        self.submit_deposit_wallet_batch(wallet, calls, nonce, deadline, signature, Some(String::new()))
            .await
    }
```

Import `B256` and `address!` as needed. In `submit_gasless_redemption_with_gas_estimation`, when `self.wallet_type == WalletType::DepositWallet`, build the call with `redeem_positions_calldata(pUSD, ...)` and route through `execute_deposit_wallet` (the legacy path uses USDC `0x2791…` as collateral, which is correct for Safe and Proxy and wrong for a Deposit Wallet). Document the collateral difference on both.

Crate docs in `lib.rs`: in the header list, add a third bullet
`//! - **Deposit Wallets** — Polymarket's smart account (default since 2026-05-04); an owner can authorize *session keys* that trade but cannot withdraw`,
and after the existing `## Example` block add:

```rust
//! ## Deposit Wallets and session keys
//!
//! With the owner's key in the process, authorize a session key and wait for the
//! relayer to confirm it:
//!
//! ```no_run
//! use polyoxide_core::SessionSignerScope;
//! use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient, WalletType};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = BuilderConfig::new("key".into(), "secret".into(), Some("passphrase".into()));
//! let owner = BuilderAccount::new("0xprivatekey...", Some(config))?;
//! let wallet = "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse()?;
//! let client = RelayClient::builder()?
//!     .with_account(owner)
//!     .wallet_type(WalletType::DepositWallet)
//!     .deposit_wallet(wallet)
//!     .build()?;
//!
//! let session_key = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse()?;
//! let submitted = client
//!     .authorize_session_signer(session_key, vec![SessionSignerScope::Clob])
//!     .await?;
//! loop {
//!     let tx = client.get_gasless_transaction(&submitted.transaction_id).await?;
//!     if tx.state.is_terminal() {
//!         break;
//!     }
//!     tokio::time::sleep(std::time::Duration::from_secs(2)).await;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! With the owner's key in an external wallet, the client only submits. It needs
//! Builder credentials but no key:
//!
//! ```no_run
//! use polyoxide_core::SessionSignerScope;
//! use polyoxide_relay::{AuthConfig, BuilderConfig, RelayClient, WalletType};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let auth = AuthConfig::Builder(BuilderConfig::new("key".into(), "secret".into(), None));
//! let client = RelayClient::builder()?.with_auth(auth).build()?;
//! let owner = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse()?;
//! let wallet = "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".parse()?;
//! let session_key = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse()?;
//!
//! let nonce = client.get_execute_params(owner, WalletType::DepositWallet).await?;
//! let deadline = polyoxide_core::current_timestamp() + polyoxide_relay::DEFAULT_BATCH_DEADLINE_SECS;
//! let (typed_data, request) = client.authorize_session_signer_typed_data(
//!     wallet, session_key, vec![SessionSignerScope::Clob], nonce, deadline,
//! )?;
//! // Hand `typed_data` to the wallet (`eth_signTypedData_v4`) and get its signature back.
//! # let signature = String::new();
//! let submitted = client
//!     .submit_session_signer_authorization(&request, &signature, "my-idempotency-key")
//!     .await?;
//! # let _ = submitted;
//! # Ok(())
//! # }
//! ```
```

README: after the "Execute Arbitrary Transactions" section, add:

````markdown
### Deposit Wallets

```rust
use polyoxide_relay::{RelayClient, WalletKind, WalletType};

# use polyoxide_relay::{BuilderAccount, BuilderConfig};
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let config = BuilderConfig::new("key".into(), "secret".into(), None);
# let account = BuilderAccount::new("0xprivatekey...", Some(config))?;
let owner = account.address();
// Which wallet does this key own? Probes the relayer for both Deposit Wallet
// generations and the Safe.
let probe = RelayClient::builder()?.build()?;
let wallet = match probe.resolve_wallet(owner).await? {
    WalletKind::DepositWallet(address) => address,
    other => return Err(format!("not a Deposit Wallet: {other:?}").into()),
};

let client = RelayClient::builder()?
    .with_account(account)
    .wallet_type(WalletType::DepositWallet)
    .deposit_wallet(wallet)
    .build()?;

// The four approvals a fresh Deposit Wallet needs before trading, in one batch.
let calls = client.deposit_wallet_trading_approvals()?;
let nonce = client.get_execute_params(owner, WalletType::DepositWallet).await?;
let deadline = polyoxide_core::current_timestamp() + polyoxide_relay::DEFAULT_BATCH_DEADLINE_SECS;
let typed_data = client.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline);
# let _ = typed_data;
# Ok(())
# }
```
````

`polyoxide-core` must be usable from the README doctest; it is a regular dependency, so `polyoxide_core::current_timestamp` resolves.

- [ ] **Step 3: Run every gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p polyoxide-core -p polyoxide-clob -p polyoxide-relay --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
cargo check --workspace --all-features
cargo test --workspace --all-features
```
Expected: each exits 0. Report the workspace totals.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-relay/src/client.rs polyoxide-relay/src/lib.rs polyoxide-relay/README.md polyoxide-relay/tests/mock_api.rs
git commit -m "feat(relay): Deposit Wallet redemption pair; document the Deposit Wallet and session-key surface"
```

---

## Not in this plan

- `WALLET_CREATE` (deploying a new Deposit Wallet).
- The `#[ignore]` live round trip, `docs/specs/session-keys/`, the `CLAUDE.md` pointer and the handoff status lines (plan 3).
- Python bindings, CLI, and the 0.33.0 release.
