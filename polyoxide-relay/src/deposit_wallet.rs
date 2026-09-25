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

fn to_protocol(
    wallet: Address,
    calls: &[DepositWalletCall],
    nonce: u64,
    deadline: u64,
) -> protocol::Batch {
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
pub fn batch_digest(
    chain_id: u64,
    wallet: Address,
    calls: &[DepositWalletCall],
    nonce: u64,
    deadline: u64,
) -> B256 {
    to_protocol(wallet, calls, nonce, deadline).eip712_signing_hash(&domain(chain_id, wallet))
}

/// A call's `value` for the typed-data JSON, exact at any size.
///
/// Up to `u64::MAX` wei (about 18.4 native tokens) it is a JSON number, the way
/// py-sdk emits it, so the fixtures match. Above that it is a decimal string.
/// Without serde_json's `arbitrary_precision` feature a JSON number holds at
/// most a `u64` or an `f64`, and an `f64` would round the value, so the JSON
/// would describe a different batch than `batch_digest` hashes and a wallet
/// signing it would produce a signature the relayer rejects. eth-sig-util,
/// ethers and viem all accept a decimal string for a `uint256` field. py-sdk
/// emits a Python int instead, so this path does not match its bytes.
fn value_json(value: U256) -> serde_json::Value {
    match u64::try_from(value) {
        Ok(v) => serde_json::Value::from(v),
        Err(_) => serde_json::Value::String(value.to_string()),
    }
}

/// The batch as EIP-712 JSON for `eth_signTypedData_v4`.
///
/// It matches py-sdk's `build_deposit_wallet_typed_data` field for field, with
/// the same array order and the same value representations, with two exceptions.
/// Each call's `data` is rendered as `0x` hex, where py-sdk returns raw bytes;
/// the fixture's hex strings come from the capture script. Object key order is
/// not preserved: keys serialize sorted rather than in insertion order, which
/// EIP-712 signers ignore, since they hash the `types` arrays.
///
/// Each call's `value` is a JSON number up to `u64::MAX` wei and a decimal
/// string above that, so the JSON always describes exactly the batch that
/// `batch_digest` hashes. Above `u64::MAX` this departs from py-sdk, which
/// emits an integer.
pub fn batch_typed_data(
    chain_id: u64,
    wallet: Address,
    calls: &[DepositWalletCall],
    nonce: u64,
    deadline: u64,
) -> serde_json::Value {
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
                "value": value_json(c.value),
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
    protocol::revokeSessionSignerCall {
        sessionSigner: session_signer,
    }
    .abi_encode()
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
pub fn redeem_positions_calldata(
    collateral: Address,
    condition_id: B256,
    index_sets: &[U256],
) -> Vec<u8> {
    protocol::redeemPositionsCall {
        collateral,
        parentCollectionId: B256::ZERO,
        conditionId: condition_id,
        indexSets: index_sets.to_vec(),
    }
    .abi_encode()
}

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
                data: hex::decode(c["data"].as_str().unwrap().trim_start_matches("0x"))
                    .unwrap()
                    .into(),
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
    fn batch_digest_matches_py_sdk_for_every_batch() {
        let v = vectors();
        // multi_batch has two calls, the second with value 1, so Call[] hashing and
        // the value field are exercised, not just single zero-value calls.
        for name in [
            "approval_batch",
            "authorize_batch",
            "revoke_batch",
            "redeem_batch",
            "multi_batch",
            "redeem_adapter_batch",
            "redeem_neg_risk_batch",
        ] {
            let (calls, nonce, deadline) = batch_from(&v[name]);
            let expected: B256 = v[name]["digest"].as_str().unwrap().parse().unwrap();
            assert_eq!(
                batch_digest(137, wallet(), &calls, nonce, deadline),
                expected,
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn signing_the_digest_reproduces_py_sdk_signatures() {
        let v = vectors();
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        for name in [
            "approval_batch",
            "authorize_batch",
            "revoke_batch",
            "redeem_batch",
            "multi_batch",
            "redeem_adapter_batch",
            "redeem_neg_risk_batch",
        ] {
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
    fn typed_data_json_equals_py_sdk_for_the_authorize_multi_and_adapter_batches() {
        let v = vectors();
        for name in [
            "authorize_batch",
            "multi_batch",
            "redeem_adapter_batch",
            "redeem_neg_risk_batch",
        ] {
            let (calls, nonce, deadline) = batch_from(&v[name]);
            let json = batch_typed_data(137, wallet(), &calls, nonce, deadline);
            assert_eq!(json, v[name]["typed_data"], "{name}");
        }
    }

    #[test]
    fn session_envelope_reproduces_py_sdk_bytes() {
        let v = vectors();
        let session: Address = v["session_signer"].as_str().unwrap().parse().unwrap();
        let wrapped = wrap_session_signer(
            session,
            &hex_bytes(v["approval_batch"]["signature"].as_str().unwrap()),
        );
        assert_eq!(wrapped.len(), 256);
        assert_eq!(
            wrapped,
            hex_bytes(v["approval_batch"]["session_signature"].as_str().unwrap())
        );
    }

    #[test]
    fn calldata_encoders_match_py_sdk() {
        let v = vectors();
        let session: Address = v["session_signer"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            format!(
                "0x{}",
                hex::encode(authorize_session_signer_calldata(session, 1815534000))
            ),
            v["authorize_batch"]["calls"][0]["data"].as_str().unwrap()
        );
        assert_eq!(
            format!("0x{}", hex::encode(revoke_session_signer_calldata(session))),
            v["revoke_batch"]["calls"][0]["data"].as_str().unwrap()
        );
        assert_eq!(
            format!(
                "0x{}",
                hex::encode(erc20_approve_calldata(
                    address!("E111180000d2663C0091e4f400237545B87B996B"),
                    alloy::primitives::U256::MAX
                ))
            ),
            v["approval_batch"]["calls"][0]["data"].as_str().unwrap()
        );
        assert_eq!(
            format!(
                "0x{}",
                hex::encode(erc1155_set_approval_for_all_calldata(
                    address!("E111180000d2663C0091e4f400237545B87B996B"),
                    true
                ))
            ),
            v["multi_batch"]["calls"][1]["data"].as_str().unwrap()
        );
        let condition: B256 = "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51"
            .parse()
            .unwrap();
        assert_eq!(
            format!(
                "0x{}",
                hex::encode(redeem_positions_calldata(
                    address!("C011a7E12a19f7B1f670d46F03B03f3342E82DFB"),
                    condition,
                    &[
                        alloy::primitives::U256::from(1),
                        alloy::primitives::U256::from(2)
                    ]
                ))
            ),
            v["redeem_batch"]["calls"][0]["data"].as_str().unwrap()
        );
    }

    #[test]
    fn a_value_above_u64_is_an_exact_decimal_string() {
        // No fixture reaches the string branch: every fixture value fits in u64.
        let fits = value_json(alloy::primitives::U256::from(u64::MAX));
        assert!(fits.is_number(), "{fits}");
        assert_eq!(fits, serde_json::json!(u64::MAX));
        let above =
            value_json(alloy::primitives::U256::from(u64::MAX) + alloy::primitives::U256::from(1));
        assert_eq!(above, serde_json::json!("18446744073709551616"));
        let max = value_json(alloy::primitives::U256::MAX);
        assert_eq!(
            max,
            serde_json::json!(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
        );
    }

    #[test]
    fn session_lifetime_is_the_venues_fixed_value() {
        assert_eq!(
            SESSION_KEY_LIFETIME_SECS,
            vectors()["session_key_lifetime_secs"].as_u64().unwrap()
        );
        assert_eq!(DEFAULT_BATCH_DEADLINE_SECS, 600);
    }
}
