//! Pure CREATE2 derivations of the account wallets a signer may own, and the
//! resolver's answer type.
//!
//! Every function here is deterministic and network-free, pinned to addresses
//! produced by Polymarket's `py-sdk` (`tests/fixtures/session_keys/relay_vectors.json`).
//! Whether a derived wallet exists on chain is a separate question answered by
//! [`crate::RelayClient::resolve_wallet`].

use alloy::primitives::{keccak256, Address, B256};
use alloy::sol_types::SolValue;

use crate::config::ContractConfig;
use crate::error::RelayError;

// The proxy-wallet init code, as `polymarket/py-sdk` `wallet.py` builds it:
// `PROXY_INIT_PREFIX ‖ factory ‖ PROXY_INIT_MIDDLE ‖ implementation ‖ PROXY_INIT_SUFFIX`.
const PROXY_INIT_PREFIX: [u8; 13] = alloy::primitives::hex!("3d3d606380380380913d393d73");
const PROXY_INIT_MIDDLE: [u8; 31] =
    alloy::primitives::hex!("5af4602a57600080fd5b602d8060366000396000f3363d3d373d3d3d363d73");
const PROXY_INIT_SUFFIX: [u8; 83] = alloy::primitives::hex!(
    "5af43d82803e903d91602b57fd5bf352e831dd"
    "0000000000000000000000000000000000000000000000000000000000000020"
    "0000000000000000000000000000000000000000000000000000000000000000"
);

// ERC-1967 minimal proxy pieces (Solady), split exactly as py-sdk splits them.
const ERC1967_CONST1: [u8; 32] =
    alloy::primitives::hex!("cc3735a920a3ca505d382bbc545af43d6000803e6038573d6000fd5b3d6000f3");
const ERC1967_CONST2: [u8; 32] =
    alloy::primitives::hex!("5155f3363d3d373d3d363d7f360894a13ba1a3210667c828492db98dca3e2076");
const ERC1967_PREFIX_BASE: u128 = 0x61003D3D8160233D3973;
const ERC1967_BEACON_CONST1: [u8; 32] =
    alloy::primitives::hex!("b3582b35133d50545afa5036515af43d6000803e604d573d6000fd5b3d6000f3");
const ERC1967_BEACON_CONST2: [u8; 32] =
    alloy::primitives::hex!("1b60e01b36527fa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6c");
const ERC1967_BEACON_CONST3: [u8; 23] =
    alloy::primitives::hex!("60195155f3363d3d373d3d363d602036600436635c60da");
const ERC1967_BEACON_PREFIX_BASE: u128 = 0x6100523D8160233D3973;

/// What [`crate::RelayClient::resolve_wallet`] found deployed for an owner.
///
/// The resolver returns `Option<WalletKind>`, where `None` means nothing is
/// deployed for the owner.
///
/// [`WalletKind::Proxy`] is found by asking `/deployed` with `type=PROXY`, as
/// py-sdk 0.11.0 does, although the published relayer spec enumerates only `SAFE`
/// and `WALLET`; `docs/specs/session-keys/OBSERVED.md` records what the host
/// answers. A Proxy auto-deploys on first use, so a fresh Proxy account resolves
/// to `None` until its first transaction; use [`derive_proxy`] when you already
/// know the account is a Proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WalletKind {
    /// A deployed Deposit Wallet (either generation).
    DepositWallet(Address),
    /// A deployed Gnosis Safe.
    Safe(Address),
    /// A deployed Polymarket proxy wallet.
    Proxy(Address),
}

impl WalletKind {
    /// The deployed wallet's address.
    pub fn address(&self) -> Address {
        match self {
            Self::DepositWallet(a) | Self::Safe(a) | Self::Proxy(a) => *a,
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
    create2(cfg.safe_factory, salt, cfg.safe_init_code_hash)
}

/// The Proxy wallet a signer owns. Salt is `keccak256(abi.encodePacked(owner))`.
pub fn derive_proxy(owner: Address, cfg: &ContractConfig) -> Result<Address, RelayError> {
    let factory = cfg
        .proxy_factory
        .ok_or_else(|| RelayError::Api("Proxy wallet not supported on this chain".to_string()))?;
    let implementation = cfg.proxy_implementation.ok_or_else(|| {
        RelayError::Api("Proxy implementation not configured for this chain".to_string())
    })?;
    let mut bytecode = Vec::with_capacity(
        PROXY_INIT_PREFIX.len() + 20 + PROXY_INIT_MIDDLE.len() + 20 + PROXY_INIT_SUFFIX.len(),
    );
    bytecode.extend_from_slice(&PROXY_INIT_PREFIX);
    bytecode.extend_from_slice(factory.as_slice());
    bytecode.extend_from_slice(&PROXY_INIT_MIDDLE);
    bytecode.extend_from_slice(implementation.as_slice());
    bytecode.extend_from_slice(&PROXY_INIT_SUFFIX);
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
    debug_assert!(prefix >> 80 == 0, "ERC-1967 prefix must fit in 10 bytes");
    let bytes = prefix.to_be_bytes();
    let mut out = [0u8; 10];
    out.copy_from_slice(&bytes[6..16]);
    out
}

/// The UUPS-generation Deposit Wallet (deployed before 2026-06-29).
pub fn derive_deposit_wallet_uups(
    owner: Address,
    cfg: &ContractConfig,
) -> Result<Address, RelayError> {
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
pub fn derive_deposit_wallet_beacon(
    owner: Address,
    cfg: &ContractConfig,
) -> Result<Address, RelayError> {
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
            assert_eq!(
                derive_deposit_wallet_uups(owner, &cfg).unwrap(),
                expected(who, "uups"),
                "{who} uups"
            );
            assert_eq!(
                derive_deposit_wallet_beacon(owner, &cfg).unwrap(),
                expected(who, "beacon"),
                "{who} beacon"
            );
            assert_eq!(
                derive_safe(owner, &cfg),
                expected(who, "safe"),
                "{who} safe"
            );
            assert_eq!(
                derive_proxy(owner, &cfg).unwrap(),
                expected(who, "proxy"),
                "{who} proxy"
            );
        }
    }

    #[test]
    fn contract_config_matches_the_fixture_constants() {
        let cfg = get_contract_config(137).unwrap();
        let c = &vectors()["config"];
        let addr = |k: &str| -> Address { c[k].as_str().unwrap().parse().unwrap() };
        assert_eq!(
            cfg.deposit_wallet_factory,
            Some(addr("deposit_wallet_factory"))
        );
        assert_eq!(
            cfg.deposit_wallet_beacon,
            Some(addr("deposit_wallet_beacon"))
        );
        assert_eq!(
            cfg.deposit_wallet_implementation,
            Some(addr("deposit_wallet_implementation"))
        );
        assert_eq!(cfg.proxy_factory, Some(addr("proxy_factory")));
        assert_eq!(cfg.proxy_implementation, Some(addr("proxy_implementation")));
        assert_eq!(cfg.safe_factory, addr("safe_factory"));
        assert_eq!(
            cfg.safe_init_code_hash.to_string(),
            c["safe_init_code_hash"].as_str().unwrap()
        );
    }

    #[test]
    fn signer_one_uups_is_the_clob_fixture_wallet() {
        // The Deposit Wallet used throughout the clob order vectors is signer 0x…01's UUPS wallet.
        let cfg = get_contract_config(137).unwrap();
        assert_eq!(
            derive_deposit_wallet_uups(address!("0000000000000000000000000000000000000001"), &cfg)
                .unwrap(),
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
        assert_eq!(WalletKind::DepositWallet(a).address(), a);
        assert_eq!(WalletKind::Safe(a).address(), a);
        assert_eq!(WalletKind::Proxy(a).address(), a);
    }
}
