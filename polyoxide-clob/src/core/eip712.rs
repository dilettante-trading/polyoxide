use alloy::{
    primitives::{keccak256, Address, B256, U256},
    signers::Signer as AlloySigner,
    sol,
    sol_types::{Eip712Domain, SolStruct},
};

use crate::{
    account::{DepositWalletRole, SigningTarget},
    core::chain::Chain,
    error::ClobError,
    types::{Order as ClobOrder, SignatureType},
};

mod protocol {
    use super::*;
    sol! {
        #[derive(Debug, PartialEq, Eq)]
        struct EIP712Domain {
            string name;
            string version;
            uint256 chainId;
            address verifyingContract;
        }

        #[derive(Debug, PartialEq, Eq)]
        struct Order {
            uint256 salt;
            address maker;
            address signer;
            uint256 tokenId;
            uint256 makerAmount;
            uint256 takerAmount;
            uint8 side;
            uint8 signatureType;
            uint256 timestamp;
            bytes32 metadata;
            bytes32 builder;
        }

        /// ERC-7739 envelope a Deposit Wallet validates (signature type 3).
        ///
        /// `contents` is the V2 [`Order`]; the remaining fields are the *wallet's*
        /// EIP-712 domain (`DepositWallet` v1, verifying contract = the wallet).
        /// The envelope is hashed under the *exchange's* domain separator. Keep the
        /// field order: the type string is part of what the contract checks.
        #[derive(Debug, PartialEq, Eq)]
        struct TypedDataSign {
            Order contents;
            string name;
            string version;
            uint256 chainId;
            address verifyingContract;
            bytes32 salt;
        }

        /// L1 authentication struct.
        ///
        /// Must hash as
        /// `ClobAuth(address address,string timestamp,uint256 nonce,string message)`
        /// to match the counterparty. Note `timestamp` is a **string**, not a
        /// uint, and `message` is a fixed constant — the timestamp and nonce
        /// are separate fields rather than text interpolated into it.
        #[derive(Debug, PartialEq, Eq)]
        struct ClobAuth {
            address address;
            string timestamp;
            uint256 nonce;
            string message;
        }
    }
}

/// The fixed `message` field of [`protocol::ClobAuth`].
///
/// A constant, not a template: the timestamp and nonce are their own struct
/// fields.
const CLOB_AUTH_MESSAGE: &str = "This message attests that I control the given wallet";

/// EIP-712 domain for L1 auth.
///
/// Carries **no `verifyingContract`**. The counterparty builds this domain from
/// name, version, and chainId alone, so it hashes as
/// `EIP712Domain(string name,string version,uint256 chainId)`. Passing a zero
/// address instead would add a fourth field to the type string and change the
/// separator — which is why this cannot reuse the fixed-shape
/// [`protocol::EIP712Domain`] that order signing needs. [`Eip712Domain`] omits
/// `None` fields from the encoded type.
fn clob_auth_domain(chain_id: u64) -> Eip712Domain {
    Eip712Domain {
        name: Some("ClobAuthDomain".into()),
        version: Some("1".into()),
        chain_id: Some(U256::from(chain_id)),
        verifying_contract: None,
        salt: None,
    }
}

/// The L1 auth message as EIP-712 JSON for `eth_signTypedData_v4`.
///
/// Hand this to an external wallet, then pass the signature it returns to
/// [`crate::Clob::create_api_key_with_signature`] or
/// [`crate::Clob::derive_api_key_with_signature`]. It is exactly what
/// [`sign_clob_auth`] signs: `ClobAuthDomain` v1 with no verifying contract,
/// `timestamp` as a decimal string, and the fixed attestation message.
pub fn clob_auth_typed_data(
    address: Address,
    chain_id: u64,
    timestamp: u64,
    nonce: u32,
) -> serde_json::Value {
    serde_json::json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" }
            ],
            "ClobAuth": [
                { "name": "address", "type": "address" },
                { "name": "timestamp", "type": "string" },
                { "name": "nonce", "type": "uint256" },
                { "name": "message", "type": "string" }
            ]
        },
        "primaryType": "ClobAuth",
        "domain": { "name": "ClobAuthDomain", "version": "1", "chainId": chain_id },
        "message": {
            "address": address.to_string(),
            "timestamp": timestamp.to_string(),
            "nonce": nonce,
            "message": CLOB_AUTH_MESSAGE
        }
    })
}

/// Convert a CLOB order to the EIP-712 protocol struct for hashing/signing.
fn order_to_protocol(order: &ClobOrder) -> Result<protocol::Order, ClobError> {
    // The venue requires a Deposit Wallet order to be made *and* signed by the
    // wallet itself; the key that produces the signature is named elsewhere
    // (in the ERC-7739 / session envelope), not in these fields.
    if order.signature_type == SignatureType::Poly1271 && order.maker != order.signer {
        return Err(ClobError::Crypto(
            "signature type 3 (Poly1271) requires maker == signer == the Deposit Wallet".into(),
        ));
    }
    Ok(protocol::Order {
        salt: U256::from_str_radix(&order.salt, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid salt: {}", e)))?,
        maker: order.maker,
        signer: order.signer,
        tokenId: U256::from_str_radix(&order.token_id, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid token_id: {}", e)))?,
        makerAmount: U256::from_str_radix(&order.maker_amount, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid maker_amount: {}", e)))?,
        takerAmount: U256::from_str_radix(&order.taker_amount, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid taker_amount: {}", e)))?,
        side: match order.side {
            crate::types::OrderSide::Buy => 0,
            crate::types::OrderSide::Sell => 1,
        },
        signatureType: order.signature_type as u8,
        timestamp: U256::from_str_radix(&order.timestamp, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid timestamp: {}", e)))?,
        metadata: order.metadata,
        builder: order.builder,
    })
}

/// The exchange an order is settled on, from the chain's contract table.
pub(crate) fn exchange_address(chain_id: u64, neg_risk: bool) -> Result<Address, ClobError> {
    let chain = Chain::from_chain_id(chain_id)
        .ok_or_else(|| ClobError::Crypto(format!("Unsupported chain ID: {}", chain_id)))?;
    let contracts = chain.contracts();
    Ok(if neg_risk {
        contracts.neg_risk_exchange
    } else {
        contracts.exchange
    })
}

/// The `Polymarket CTF Exchange` v2 domain for one exchange contract.
fn exchange_domain(chain_id: u64, exchange: Address) -> protocol::EIP712Domain {
    protocol::EIP712Domain {
        name: "Polymarket CTF Exchange".to_string(),
        version: "2".to_string(),
        chainId: U256::from(chain_id),
        verifyingContract: exchange,
    }
}

/// `keccak256(0x1901 ‖ domain_separator ‖ struct_hash)`.
fn eip712_digest(domain_separator: B256, struct_hash: B256) -> B256 {
    let mut message = Vec::with_capacity(66);
    message.extend_from_slice(b"\x19\x01");
    message.extend_from_slice(domain_separator.as_slice());
    message.extend_from_slice(struct_hash.as_slice());
    keccak256(&message)
}

/// Compute the EIP-712 digest for a plain (type 0–2) order, without signing.
fn compute_order_digest(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError> {
    if order.signature_type == SignatureType::Poly1271 {
        return Err(ClobError::Crypto(
            "signature type 3 (Poly1271) is signed as an ERC-7739 envelope for a Deposit \
             Wallet; use sign_order_as with SigningTarget::DepositWallet"
                .into(),
        ));
    }
    let exchange = exchange_address(chain_id, order.neg_risk)?;
    let domain = exchange_domain(chain_id, exchange);
    let struct_hash = order_to_protocol(order)?.eip712_hash_struct();
    Ok(eip712_digest(domain.eip712_hash_struct(), struct_hash))
}

/// Sign an order with EIP-712
pub async fn sign_order<S: AlloySigner + ?Sized>(
    order: &ClobOrder,
    signer: &S,
    chain_id: u64,
) -> Result<String, ClobError> {
    let digest = compute_order_digest(order, chain_id)?;
    let signature = signer.sign_hash(&digest).await?;
    Ok(format!("0x{}", hex::encode(signature.as_bytes())))
}

/// Deposit Wallet (signature type 3) signing primitives.
///
/// A Deposit Wallet is an ERC-1271 smart account that validates order
/// signatures through ERC-7739: the key signs a `TypedDataSign` envelope
/// carrying the order and the wallet's own domain, hashed under the exchange's
/// domain separator, and the signature bytes are followed by a trailer that
/// lets the wallet rebuild the digest. A session key additionally wraps the
/// result in an ERC-6492-style envelope naming itself.
///
/// Every function here is pure and pinned byte-for-byte to vectors from
/// Polymarket's `py-sdk` (`tests/fixtures/session_keys/order_vectors.json`).
/// These are low-level pieces; [`sign_order_as`] is the
/// function that also enforces the venue's rules (signature type 3, and
/// `maker == signer == wallet`) before using them.
pub(crate) mod deposit_wallet {
    use alloy::{
        primitives::{hex, Address, Bytes, B256, U256},
        sol_types::{SolStruct, SolValue},
    };

    use super::{eip712_digest, exchange_domain, order_to_protocol, protocol};
    use crate::{error::ClobError, types::Order as ClobOrder};

    /// EIP-712 domain name of every Deposit Wallet.
    pub const DOMAIN_NAME: &str = "DepositWallet";
    /// EIP-712 domain version of every Deposit Wallet.
    pub const DOMAIN_VERSION: &str = "1";

    /// The 32-byte suffix that marks a session-signer envelope (`0x6492` × 16).
    pub const SESSION_SIGNER_MAGIC: [u8; 32] =
        hex!("6492649264926492649264926492649264926492649264926492649264926492");

    /// The digest a Deposit Wallet key signs for `order`.
    ///
    /// `exchange` is the verifying contract of the outer (app) domain and
    /// `wallet` the Deposit Wallet named inside the envelope. This computes
    /// the digest for whatever it is given: it does not check that
    /// `order.signature_type` is 3 or that `order.maker` is `wallet`; only
    /// the `maker == signer` rule is enforced, by the order conversion, and
    /// only for signature type 3. `sign_order_as` applies the other two.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "sign_order_as calls envelope_digest_from; this is the vector-pinned entry point"
        )
    )]
    pub fn envelope_digest(
        order: &ClobOrder,
        chain_id: u64,
        exchange: Address,
        wallet: Address,
    ) -> Result<B256, ClobError> {
        let contents = order_to_protocol(order)?;
        let app_domain_separator = exchange_domain(chain_id, exchange).eip712_hash_struct();
        Ok(envelope_digest_from(
            contents,
            chain_id,
            app_domain_separator,
            wallet,
        ))
    }

    /// [`envelope_digest`] from an already-converted order and an already-hashed
    /// exchange domain, so a caller that also builds the ERC-7739 trailer signs
    /// exactly the values the trailer carries.
    pub(super) fn envelope_digest_from(
        contents: protocol::Order,
        chain_id: u64,
        app_domain_separator: B256,
        wallet: Address,
    ) -> B256 {
        let envelope = protocol::TypedDataSign {
            contents,
            name: DOMAIN_NAME.to_string(),
            version: DOMAIN_VERSION.to_string(),
            chainId: U256::from(chain_id),
            verifyingContract: wallet,
            salt: B256::ZERO,
        };
        eip712_digest(app_domain_separator, envelope.eip712_hash_struct())
    }

    /// Append the ERC-7739 trailer to an inner signature.
    ///
    /// `inner ‖ appDomainSeparator ‖ contentsHash ‖ bytes(ORDER_TYPE) ‖ uint16(len(ORDER_TYPE))`,
    /// where `ORDER_TYPE` is the V2 order type string (186 bytes).
    pub fn wrap_erc7739(inner: &[u8], app_domain_separator: B256, contents_hash: B256) -> Vec<u8> {
        let type_string = protocol::Order::eip712_encode_type();
        // 186 bytes, pinned by `v2_order_type_string_matches_contract`.
        let type_len = u16::try_from(type_string.len()).expect("order type string fits in u16");
        let mut out = Vec::with_capacity(inner.len() + 64 + type_string.len() + 2);
        out.extend_from_slice(inner);
        out.extend_from_slice(app_domain_separator.as_slice());
        out.extend_from_slice(contents_hash.as_slice());
        out.extend_from_slice(type_string.as_bytes());
        out.extend_from_slice(&type_len.to_be_bytes());
        out
    }

    /// Wrap an ERC-7739 signature in the session-signer envelope.
    ///
    /// `abi.encode(bytes32(leftPad(session_signer)), bytes32(0), bytes(wrapped)) ‖ 0x6492…6492`.
    pub fn wrap_session_signer(session_signer: Address, wrapped: &[u8]) -> Vec<u8> {
        let signer_id = B256::left_padding_from(session_signer.as_slice());
        let mut out = (signer_id, B256::ZERO, Bytes::copy_from_slice(wrapped)).abi_encode_params();
        out.extend_from_slice(&SESSION_SIGNER_MAGIC);
        out
    }
}

/// Sign an order for the account described by `target`.
///
/// For [`SigningTarget::Eoa`], [`SigningTarget::PolyProxy`] and
/// [`SigningTarget::PolyGnosisSafe`] this is [`sign_order`]. For a
/// [`SigningTarget::DepositWallet`] it signs the ERC-7739 envelope, appends the
/// trailer, and for a session key adds the session-signer envelope naming
/// `signer.address()`. A type-3 order needs a Deposit Wallet target and vice
/// versa; mixing them is an error before any signing happens.
pub async fn sign_order_as<S: AlloySigner + ?Sized>(
    order: &ClobOrder,
    signer: &S,
    chain_id: u64,
    target: &SigningTarget,
) -> Result<String, ClobError> {
    let Some((wallet, role)) = target.deposit_wallet() else {
        if order.signature_type == SignatureType::Poly1271 {
            return Err(ClobError::validation(
                "signature type 3 (Poly1271) needs SigningTarget::DepositWallet; this account \
                 targets an EOA, proxy or Safe",
            ));
        }
        return sign_order(order, signer, chain_id).await;
    };
    if order.signature_type != SignatureType::Poly1271 {
        return Err(ClobError::validation(format!(
            "a Deposit Wallet target signs only signature type 3 orders, got {} ({})",
            order.signature_type, order.signature_type as u8
        )));
    }
    if order.maker != wallet || order.signer != wallet {
        return Err(ClobError::validation(format!(
            "Deposit Wallet orders need maker == signer == {wallet}, got maker {} signer {}",
            order.maker, order.signer
        )));
    }

    let exchange = exchange_address(chain_id, order.neg_risk)?;
    let contents = order_to_protocol(order)?;
    let contents_hash = contents.eip712_hash_struct();
    let app_domain_separator = exchange_domain(chain_id, exchange).eip712_hash_struct();
    let digest =
        deposit_wallet::envelope_digest_from(contents, chain_id, app_domain_separator, wallet);
    let inner = signer.sign_hash(&digest).await?;
    let wrapped =
        deposit_wallet::wrap_erc7739(&inner.as_bytes(), app_domain_separator, contents_hash);
    let bytes = match role {
        DepositWalletRole::Owner => wrapped,
        DepositWalletRole::SessionKey => {
            deposit_wallet::wrap_session_signer(signer.address(), &wrapped)
        }
    };
    Ok(format!("0x{}", hex::encode(bytes)))
}

/// Sign CLOB auth message for API key creation
pub async fn sign_clob_auth<S: AlloySigner + ?Sized>(
    signer: &S,
    chain_id: u64,
    timestamp: u64,
    nonce: u32,
) -> Result<String, ClobError> {
    let domain = clob_auth_domain(chain_id);

    let clob_auth = protocol::ClobAuth {
        address: signer.address(),
        // A string field upstream, so the digest covers the decimal text.
        timestamp: timestamp.to_string(),
        nonce: U256::from(nonce),
        message: CLOB_AUTH_MESSAGE.to_string(),
    };

    // Compute struct hash and domain separator
    let struct_hash = clob_auth.eip712_hash_struct();
    let domain_separator = domain.separator();

    // Compute final hash
    let digest = eip712_digest(domain_separator, struct_hash);

    // Sign the digest
    let signature = signer.sign_hash(&digest).await?;

    Ok(format!("0x{}", hex::encode(signature.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::OrderSide;
    use alloy::primitives::{address, Address};
    use alloy::signers::local::PrivateKeySigner;

    // Well-known Hardhat test private key #0 (DO NOT use in production)
    const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn test_signer() -> PrivateKeySigner {
        TEST_KEY.parse::<PrivateKeySigner>().unwrap()
    }

    fn make_test_order(neg_risk: bool) -> ClobOrder {
        let signer = test_signer();
        ClobOrder {
            salt: "123456789".to_string(),
            maker: signer.address(),
            signer: signer.address(),
            token_id: "100".to_string(),
            maker_amount: "5000000".to_string(),
            taker_amount: "10000000".to_string(),
            side: OrderSide::Buy,
            expiration: "0".to_string(),
            signature_type: SignatureType::Eoa,
            timestamp: "1700000000000".to_string(),
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::ZERO,
            neg_risk,
        }
    }

    #[test]
    fn v2_order_type_string_matches_contract() {
        use alloy::sol_types::SolStruct;
        let expected = "Order(uint256 salt,address maker,address signer,uint256 tokenId,\
uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,\
uint256 timestamp,bytes32 metadata,bytes32 builder)";
        assert_eq!(protocol::Order::eip712_encode_type(), expected);
    }

    #[tokio::test]
    async fn v2_order_signature_is_deterministic() {
        use alloy::signers::local::PrivateKeySigner;
        let signer: PrivateKeySigner =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .unwrap();
        let order = ClobOrder {
            salt: "479249096354".to_string(),
            maker: address!("0000000000000000000000000000000000000001"),
            signer: address!("0000000000000000000000000000000000000002"),
            token_id: "100".to_string(),
            maker_amount: "1000000".to_string(),
            taker_amount: "5000000".to_string(),
            side: crate::types::OrderSide::Buy,
            signature_type: crate::types::SignatureType::Eoa,
            timestamp: "1700000000000".to_string(),
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::ZERO,
            expiration: "0".to_string(),
            neg_risk: false,
        };
        let sig1 = sign_order(&order, &signer, 137).await.unwrap();
        let sig2 = sign_order(&order, &signer, 137).await.unwrap();
        assert_eq!(sig1, sig2);
        assert!(sig1.starts_with("0x") && sig1.len() == 132);
        // Pin the full EIP-712 digest end-to-end (domain v2 + V2 struct encoding) against
        // a captured golden signature for this fixed Hardhat-key order on Polygon (137).
        // A change here means the domain separator or struct encoding drifted.
        assert_eq!(
            sig1,
            "0xf631fbb8e61746f7be8f49898c4caea3ab63a576cd0a340d5c82de147d6f751d\
42ea67b4b650be23f9d3e8e9af57ef4e96b6f7031de7917a9eb746229f279c251c"
        );
    }

    #[test]
    fn order_to_protocol_accepts_poly1271_when_maker_is_signer() {
        let mut order = make_test_order(false);
        order.signature_type = SignatureType::Poly1271;
        let proto = order_to_protocol(&order).unwrap();
        assert_eq!(proto.signatureType, 3);
    }

    #[test]
    fn order_to_protocol_rejects_poly1271_when_maker_differs_from_signer() {
        let mut order = make_test_order(false);
        order.signature_type = SignatureType::Poly1271;
        order.maker = address!("57ffbc34de23124faeb8387fcd689d314e57accd");
        let err = order_to_protocol(&order).unwrap_err().to_string();
        assert!(err.contains("maker == signer"), "{err}");
    }

    #[test]
    fn compute_order_digest_rejects_poly1271() {
        let mut order = make_test_order(false);
        order.signature_type = SignatureType::Poly1271;
        let result = compute_order_digest(&order, 137);
        assert!(
            result.is_err(),
            "digest computation must reject Poly1271 orders"
        );
    }

    #[tokio::test]
    async fn sign_order_rejects_poly1271() {
        // Full signing path: a hand-built Poly1271 order cannot be signed and emitted
        // as an (invalid) ECDSA signature.
        let signer = test_signer();
        let mut order = make_test_order(false);
        order.signature_type = SignatureType::Poly1271;
        let result = sign_order(&order, &signer, 137).await;
        assert!(
            result.is_err(),
            "plain signing must reject Poly1271; it needs the ERC-7739 envelope"
        );
    }

    #[test]
    fn order_to_protocol_valid_order() {
        let order = make_test_order(false);
        let result = order_to_protocol(&order);
        assert!(result.is_ok());
        let proto = result.unwrap();
        assert_eq!(proto.salt, U256::from(123456789u64));
        assert_eq!(proto.maker, order.maker);
        assert_eq!(proto.signer, order.signer);
        assert_eq!(proto.tokenId, U256::from(100u64));
        assert_eq!(proto.makerAmount, U256::from(5000000u64));
        assert_eq!(proto.takerAmount, U256::from(10000000u64));
        assert_eq!(proto.side, 0); // Buy
        assert_eq!(proto.signatureType, 0); // Eoa
        assert_eq!(proto.timestamp, U256::from(1700000000000u64));
        assert_eq!(proto.metadata, alloy::primitives::B256::ZERO);
        assert_eq!(proto.builder, alloy::primitives::B256::ZERO);
    }

    #[test]
    fn order_to_protocol_sell_side() {
        let mut order = make_test_order(false);
        order.side = OrderSide::Sell;
        let proto = order_to_protocol(&order).unwrap();
        assert_eq!(proto.side, 1);
    }

    #[test]
    fn order_to_protocol_signature_types() {
        let mut order = make_test_order(false);

        order.signature_type = SignatureType::PolyProxy;
        assert_eq!(order_to_protocol(&order).unwrap().signatureType, 1);

        order.signature_type = SignatureType::PolyGnosisSafe;
        assert_eq!(order_to_protocol(&order).unwrap().signatureType, 2);
    }

    #[test]
    fn order_to_protocol_invalid_field() {
        let mut order = make_test_order(false);
        order.maker_amount = "not_a_number".to_string();
        assert!(order_to_protocol(&order).is_err());
    }

    #[test]
    fn domain_separator_differs_by_chain() {
        let mainnet_domain = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: address!("4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"),
        };
        let amoy_domain = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(80002u64),
            verifyingContract: address!("dFE02Eb6733538f8Ea35D585af8DE5958AD99E40"),
        };

        let mainnet_sep = mainnet_domain.eip712_hash_struct();
        let amoy_sep = amoy_domain.eip712_hash_struct();

        assert_ne!(
            mainnet_sep, amoy_sep,
            "Domain separators must differ between chains"
        );
    }

    #[test]
    fn domain_separator_differs_by_contract() {
        let regular = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: address!("4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"),
        };
        let neg_risk = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: address!("C5d563A36AE78145C45a50134d48A1215220f80a"),
        };

        assert_ne!(
            regular.eip712_hash_struct(),
            neg_risk.eip712_hash_struct(),
            "Domain separators must differ between exchange contracts"
        );
    }

    #[test]
    fn domain_separator_is_deterministic() {
        let domain1 = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: address!("4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"),
        };
        let domain2 = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: address!("4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"),
        };

        assert_eq!(
            domain1.eip712_hash_struct(),
            domain2.eip712_hash_struct(),
            "Same domain parameters must produce same separator"
        );
    }

    #[test]
    fn order_struct_hash_differs_by_side() {
        let order_buy = protocol::Order {
            salt: U256::from(1u64),
            maker: Address::ZERO,
            signer: Address::ZERO,
            tokenId: U256::from(100u64),
            makerAmount: U256::from(1000u64),
            takerAmount: U256::from(2000u64),
            side: 0,
            signatureType: 0,
            timestamp: U256::ZERO,
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::ZERO,
        };
        let order_sell = protocol::Order {
            side: 1,
            ..order_buy
        };

        assert_ne!(
            order_buy.eip712_hash_struct(),
            order_sell.eip712_hash_struct(),
            "Buy and sell orders must produce different struct hashes"
        );
    }

    #[test]
    fn order_struct_hash_differs_by_amount() {
        let order1 = protocol::Order {
            salt: U256::from(1u64),
            maker: Address::ZERO,
            signer: Address::ZERO,
            tokenId: U256::from(100u64),
            makerAmount: U256::from(1000u64),
            takerAmount: U256::from(2000u64),
            side: 0,
            signatureType: 0,
            timestamp: U256::ZERO,
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::ZERO,
        };
        let order2 = protocol::Order {
            makerAmount: U256::from(1001u64),
            ..order1
        };

        assert_ne!(
            order1.eip712_hash_struct(),
            order2.eip712_hash_struct(),
            "Orders with different amounts must produce different hashes"
        );
    }

    #[test]
    fn order_digest_uses_correct_exchange_for_neg_risk() {
        let order_regular = make_test_order(false);
        let order_neg_risk = make_test_order(true);

        let digest_regular = compute_order_digest(&order_regular, 137).unwrap();
        let digest_neg_risk = compute_order_digest(&order_neg_risk, 137).unwrap();

        assert_ne!(
            digest_regular, digest_neg_risk,
            "Regular and neg_risk orders must produce different digests"
        );
    }

    #[test]
    fn order_digest_differs_by_chain() {
        let order = make_test_order(false);

        let digest_mainnet = compute_order_digest(&order, 137).unwrap();
        let digest_amoy = compute_order_digest(&order, 80002).unwrap();

        assert_ne!(
            digest_mainnet, digest_amoy,
            "Same order on different chains must produce different digests"
        );
    }

    #[test]
    fn order_digest_differs_by_builder() {
        // The `builder` field is part of the V2 signed struct, so attribution is
        // cryptographically bound. A regression that dropped `builder` from
        // `order_to_protocol` or the `sol!` struct would make these digests equal.
        let mut order_zero = make_test_order(false);
        order_zero.builder = alloy::primitives::B256::ZERO;
        let mut order_stamped = make_test_order(false);
        order_stamped.builder = alloy::primitives::B256::from([0x11u8; 32]);

        let digest_zero = compute_order_digest(&order_zero, 137).unwrap();
        let digest_stamped = compute_order_digest(&order_stamped, 137).unwrap();

        assert_ne!(
            digest_zero, digest_stamped,
            "Orders differing only by builder must produce different digests"
        );
    }

    #[test]
    fn order_digest_differs_by_metadata() {
        // The `metadata` field is part of the V2 signed struct; varying it alone
        // must change the digest, guarding against it being dropped from signing.
        let mut order_zero = make_test_order(false);
        order_zero.metadata = alloy::primitives::B256::ZERO;
        let mut order_tagged = make_test_order(false);
        order_tagged.metadata = alloy::primitives::B256::from([0x22u8; 32]);

        let digest_zero = compute_order_digest(&order_zero, 137).unwrap();
        let digest_tagged = compute_order_digest(&order_tagged, 137).unwrap();

        assert_ne!(
            digest_zero, digest_tagged,
            "Orders differing only by metadata must produce different digests"
        );
    }

    #[test]
    fn order_digest_rejects_unsupported_chain() {
        let order = make_test_order(false);
        let result = compute_order_digest(&order, 1);
        assert!(result.is_err(), "Should reject unsupported chain ID");
    }

    #[test]
    fn order_digest_rejects_invalid_salt() {
        let mut order = make_test_order(false);
        order.salt = "not_a_number".to_string();
        let result = compute_order_digest(&order, 137);
        assert!(result.is_err(), "Should reject invalid salt");
    }

    #[test]
    fn order_digest_rejects_invalid_token_id() {
        let mut order = make_test_order(false);
        order.token_id = "abc".to_string();
        let result = compute_order_digest(&order, 137);
        assert!(result.is_err(), "Should reject invalid token_id");
    }

    #[test]
    fn order_digest_rejects_invalid_maker_amount() {
        let mut order = make_test_order(false);
        order.maker_amount = "not_a_number".to_string();
        let result = compute_order_digest(&order, 137);
        assert!(result.is_err(), "Should reject invalid maker_amount");
    }

    #[test]
    fn order_digest_is_deterministic() {
        let order = make_test_order(false);

        let digest1 = compute_order_digest(&order, 137).unwrap();
        let digest2 = compute_order_digest(&order, 137).unwrap();

        assert_eq!(digest1, digest2, "Same order must produce same digest");
    }

    #[tokio::test]
    async fn sign_order_produces_valid_hex_signature() {
        let signer = test_signer();
        let order = make_test_order(false);

        let signature = sign_order(&order, &signer, 137).await.unwrap();

        assert!(
            signature.starts_with("0x"),
            "Signature must start with 0x: {}",
            signature
        );

        let decoded = hex::decode(&signature[2..]).unwrap();
        assert_eq!(
            decoded.len(),
            65,
            "Signature must be 65 bytes, got {}",
            decoded.len()
        );
    }

    #[tokio::test]
    async fn sign_order_deterministic_for_same_inputs() {
        let signer = test_signer();
        let order = make_test_order(false);

        let sig1 = sign_order(&order, &signer, 137).await.unwrap();
        let sig2 = sign_order(&order, &signer, 137).await.unwrap();

        assert_eq!(sig1, sig2, "Same inputs must produce same signature");
    }

    #[tokio::test]
    async fn sign_order_differs_for_different_orders() {
        let signer = test_signer();
        let order1 = make_test_order(false);
        let mut order2 = make_test_order(false);
        order2.salt = "987654321".to_string();

        let sig1 = sign_order(&order1, &signer, 137).await.unwrap();
        let sig2 = sign_order(&order2, &signer, 137).await.unwrap();

        assert_ne!(
            sig1, sig2,
            "Different orders must produce different signatures"
        );
    }

    #[tokio::test]
    async fn sign_order_rejects_unsupported_chain() {
        let signer = test_signer();
        let order = make_test_order(false);

        let result = sign_order(&order, &signer, 1).await;
        assert!(result.is_err(), "Should reject unsupported chain");
    }

    #[tokio::test]
    async fn sign_clob_auth_produces_valid_signature() {
        let signer = test_signer();

        let signature = sign_clob_auth(&signer, 137, 1700000000, 42).await.unwrap();

        assert!(
            signature.starts_with("0x"),
            "Signature must start with 0x: {}",
            signature
        );
        let decoded = hex::decode(&signature[2..]).unwrap();
        assert_eq!(decoded.len(), 65, "Signature must be 65 bytes");
    }

    #[tokio::test]
    async fn sign_clob_auth_deterministic() {
        let signer = test_signer();

        let sig1 = sign_clob_auth(&signer, 137, 1700000000, 42).await.unwrap();
        let sig2 = sign_clob_auth(&signer, 137, 1700000000, 42).await.unwrap();

        assert_eq!(sig1, sig2, "Same inputs must produce same signature");
    }

    #[tokio::test]
    async fn sign_clob_auth_differs_by_timestamp() {
        let signer = test_signer();

        let sig1 = sign_clob_auth(&signer, 137, 1700000000, 42).await.unwrap();
        let sig2 = sign_clob_auth(&signer, 137, 1700000001, 42).await.unwrap();

        assert_ne!(
            sig1, sig2,
            "Different timestamps must produce different signatures"
        );
    }

    #[tokio::test]
    async fn sign_clob_auth_differs_by_nonce() {
        let signer = test_signer();

        let sig1 = sign_clob_auth(&signer, 137, 1700000000, 42).await.unwrap();
        let sig2 = sign_clob_auth(&signer, 137, 1700000000, 43).await.unwrap();

        assert_ne!(
            sig1, sig2,
            "Different nonces must produce different signatures"
        );
    }

    #[test]
    fn clob_auth_domain_uses_correct_name() {
        let domain = protocol::EIP712Domain {
            name: "ClobAuthDomain".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: Address::ZERO,
        };

        let order_domain = protocol::EIP712Domain {
            name: "Polymarket CTF Exchange".to_string(),
            version: "1".to_string(),
            chainId: U256::from(137u64),
            verifyingContract: Address::ZERO,
        };

        assert_ne!(
            domain.eip712_hash_struct(),
            order_domain.eip712_hash_struct(),
            "ClobAuthDomain and Polymarket CTF Exchange must have different domain separators"
        );
    }

    #[test]
    fn signature_type_maps_correctly_to_u8() {
        let eoa = protocol::Order {
            salt: U256::ZERO,
            maker: Address::ZERO,
            signer: Address::ZERO,
            tokenId: U256::ZERO,
            makerAmount: U256::ZERO,
            takerAmount: U256::ZERO,
            side: 0,
            signatureType: 0,
            timestamp: U256::ZERO,
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::ZERO,
        };
        let proxy = protocol::Order {
            signatureType: 1,
            ..eoa
        };
        let gnosis = protocol::Order {
            signatureType: 2,
            ..eoa
        };

        let h0 = eoa.eip712_hash_struct();
        let h1 = proxy.eip712_hash_struct();
        let h2 = gnosis.eip712_hash_struct();

        assert_ne!(h0, h1);
        assert_ne!(h1, h2);
        assert_ne!(h0, h2);
    }
}

#[cfg(test)]
mod clob_auth_reference_tests {
    use super::*;
    use alloy::signers::local::PrivateKeySigner;

    const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    /// Golden vector produced by the reference implementation
    /// (Polymarket/py-clob-client + poly_eip712_structs) for
    /// key = Hardhat #0, chain_id = 137, timestamp = 1700000000, nonce = 42.
    ///
    /// Regenerate with:
    /// ```python
    /// from poly_eip712_structs import make_domain, EIP712Struct, Address, String, Uint
    /// from eth_utils import keccak
    /// from eth_account import Account
    /// class ClobAuth(EIP712Struct):
    ///     address = Address(); timestamp = String(); nonce = Uint(); message = String()
    /// acct = Account.from_key("0x" + KEY)
    /// msg = ClobAuth(address=acct.address, timestamp="1700000000", nonce=42,
    ///                message="This message attests that I control the given wallet")
    /// digest = keccak(msg.signable_bytes(
    ///     make_domain(name="ClobAuthDomain", version="1", chainId=137)))
    /// ```
    const REFERENCE_SIGNATURE: &str = "0x8e61f918d542a48ff9433fb6cc4c172a2763ad53dda8afa07f77321e2e0a1e3055f29fac1408e0020dca977190fd4f01b1c73b5e2078be29c88a32887945fe2a1b";

    #[tokio::test]
    async fn clob_auth_struct_matches_the_reference_type_string() {
        // The counterparty hashes ClobAuth(address address,string timestamp,
        // uint256 nonce,string message). Any deviation — field order, a uint
        // timestamp instead of string, or folding the values into `message` —
        // produces a different type hash and a signature the server rejects.
        assert_eq!(
            protocol::ClobAuth::eip712_encode_type(),
            "ClobAuth(address address,string timestamp,uint256 nonce,string message)"
        );
    }

    #[test]
    fn clob_auth_domain_omits_verifying_contract() {
        // Localizes the second half of the bug: the struct can be correct and
        // the signature still rejected if the domain carries a fourth field.
        // Separators below come from poly_eip712_structs' make_domain.
        assert_eq!(
            clob_auth_domain(137).separator().to_string(),
            "0xcfc66be2a3b30464cb3b588324101f660c9a205fa76e8e5f83ee16a528e1c4cb"
        );
        assert_eq!(
            clob_auth_domain(80002).separator().to_string(),
            "0xa1df8f4e3112eaee2448fbec9ab79f68278407e49d9cf52e3dd3d9692fcac9b6"
        );
        assert!(clob_auth_domain(137).verifying_contract.is_none());
    }

    #[tokio::test]
    async fn sign_clob_auth_matches_the_reference_implementation() {
        let signer = TEST_KEY.parse::<PrivateKeySigner>().unwrap();
        let signature = sign_clob_auth(&signer, 137, 1_700_000_000, 42)
            .await
            .unwrap();
        assert_eq!(
            signature, REFERENCE_SIGNATURE,
            "signature must byte-match py-clob-client for the same inputs"
        );
    }
}

/// Deposit Wallet (signature type 3) signing, pinned byte-for-byte to vectors
/// generated by Polymarket's official `py-sdk`
/// (`scripts/capture_session_key_vectors.py`). A shape-only test would pass with
/// the two EIP-712 domains swapped; these do not.
#[cfg(test)]
mod deposit_wallet_vectors {
    use super::deposit_wallet::*;
    use super::*;
    use crate::account::{DepositWalletRole, SigningTarget};
    use crate::types::OrderSide;
    use alloy::primitives::{address, Address, B256};
    use alloy::signers::local::PrivateKeySigner;

    const VECTORS: &str = include_str!("../../tests/fixtures/session_keys/order_vectors.json");
    const ANVIL_KEY_0: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const DW: Address = address!("57ffbc34de23124faeb8387fcd689d314e57accd");
    const SESSION_SIGNER: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");

    #[derive(serde::Deserialize)]
    struct Vector {
        exchange: Address,
        envelope_digest: B256,
        app_domain_separator: B256,
        contents_hash: B256,
        inner_signature: String,
        wrapped_signature: String,
        session_signature: String,
    }

    fn vector(name: &str) -> Vector {
        let mut all: std::collections::HashMap<String, Vector> =
            serde_json::from_str(VECTORS).unwrap();
        all.remove(name)
            .unwrap_or_else(|| panic!("no vector {name}"))
    }

    /// py-sdk's golden fixture: maker = signer = the wallet, salt 1, token 1,
    /// 1_000_000 / 500_000, BUY, timestamp 0, zero metadata and builder.
    fn fixture_order() -> ClobOrder {
        ClobOrder {
            salt: "1".to_string(),
            maker: DW,
            signer: DW,
            token_id: "1".to_string(),
            maker_amount: "1000000".to_string(),
            taker_amount: "500000".to_string(),
            side: OrderSide::Buy,
            expiration: "0".to_string(),
            signature_type: SignatureType::Poly1271,
            timestamp: "0".to_string(),
            metadata: B256::ZERO,
            builder: B256::ZERO,
            neg_risk: false,
        }
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        hex::decode(s.trim_start_matches("0x")).unwrap()
    }

    #[test]
    fn typed_data_sign_type_string_is_erc7739() {
        assert_eq!(
            protocol::TypedDataSign::eip712_encode_type(),
            "TypedDataSign(Order contents,string name,string version,uint256 chainId,\
address verifyingContract,bytes32 salt)Order(uint256 salt,address maker,address signer,\
uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,\
uint256 timestamp,bytes32 metadata,bytes32 builder)"
        );
    }

    #[test]
    fn envelope_digest_matches_py_sdk_for_both_exchanges() {
        for name in ["v1_exchange", "v2_exchange"] {
            let v = vector(name);
            let digest = envelope_digest(&fixture_order(), 137, v.exchange, DW).unwrap();
            assert_eq!(digest, v.envelope_digest, "{name}");
        }
    }

    #[test]
    fn swapping_the_two_domains_changes_the_digest() {
        // A fixture check, not a test of production code: it proves the vector
        // distinguishes the two layouts, so matching it means the layout is right.
        // `envelope_digest_matches_py_sdk_for_both_exchanges` tests the code.
        // The handoff document once had the exchange domain inside the message and
        // the DepositWallet domain outside. That layout must not reproduce the vector.
        let v = vector("v1_exchange");
        let order = fixture_order();
        let contents = order_to_protocol(&order).unwrap();
        let swapped = protocol::TypedDataSign {
            contents,
            name: "Polymarket CTF Exchange".into(),
            version: "2".into(),
            chainId: U256::from(137),
            verifyingContract: v.exchange,
            salt: B256::ZERO,
        };
        let wrong_domain = protocol::EIP712Domain {
            name: DOMAIN_NAME.into(),
            version: DOMAIN_VERSION.into(),
            chainId: U256::from(137),
            verifyingContract: DW,
        };
        let wrong = eip712_digest(
            wrong_domain.eip712_hash_struct(),
            swapped.eip712_hash_struct(),
        );
        assert_ne!(wrong, v.envelope_digest);
    }

    #[test]
    fn app_domain_separator_and_contents_hash_match_py_sdk() {
        for name in ["v1_exchange", "v2_exchange"] {
            let v = vector(name);
            let sep = exchange_domain(137, v.exchange).eip712_hash_struct();
            assert_eq!(sep, v.app_domain_separator, "{name} separator");
            let contents = order_to_protocol(&fixture_order())
                .unwrap()
                .eip712_hash_struct();
            assert_eq!(contents, v.contents_hash, "{name} contents");
        }
    }

    #[test]
    fn wrap_erc7739_reproduces_py_sdk_bytes() {
        let v = vector("v1_exchange");
        let wrapped = wrap_erc7739(
            &hex_bytes(&v.inner_signature),
            v.app_domain_separator,
            v.contents_hash,
        );
        assert_eq!(wrapped.len(), 317);
        assert_eq!(wrapped, hex_bytes(&v.wrapped_signature));
    }

    #[test]
    fn wrap_session_signer_reproduces_py_sdk_bytes() {
        let v = vector("v1_exchange");
        let session = wrap_session_signer(SESSION_SIGNER, &hex_bytes(&v.wrapped_signature));
        assert_eq!(session.len(), 480);
        assert_eq!(session, hex_bytes(&v.session_signature));
        assert_eq!(&session[448..], &SESSION_SIGNER_MAGIC);
    }

    #[tokio::test]
    async fn sign_order_as_owner_reproduces_py_sdk_signature() {
        // v2_exchange: the chain table resolves to this exchange for neg_risk = false.
        let v = vector("v2_exchange");
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let target = SigningTarget::DepositWallet {
            wallet: DW,
            role: DepositWalletRole::Owner,
        };
        let sig = sign_order_as(&fixture_order(), &signer, 137, &target)
            .await
            .unwrap();
        assert_eq!(sig, v.wrapped_signature);
    }

    #[tokio::test]
    async fn sign_order_as_session_key_reproduces_py_sdk_signature() {
        // Anvil key #0 signs while the vector's envelope names Anvil #1, so the
        // envelope differs from the vector only in the 32-byte signer id.
        let v = vector("v2_exchange");
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let target = SigningTarget::DepositWallet {
            wallet: DW,
            role: DepositWalletRole::SessionKey,
        };
        let sig = sign_order_as(&fixture_order(), &signer, 137, &target)
            .await
            .unwrap();
        let bytes = hex_bytes(&sig);
        let expected = hex_bytes(&v.session_signature);
        assert_eq!(bytes.len(), expected.len());
        assert_eq!(&bytes[..12], &expected[..12]);
        assert_eq!(&bytes[12..32], signer.address().as_slice());
        assert_eq!(&bytes[32..], &expected[32..]);
    }

    #[tokio::test]
    async fn sign_order_as_rejects_a_type3_order_on_a_non_deposit_wallet_target() {
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let err = sign_order_as(&fixture_order(), &signer, 137, &SigningTarget::Eoa)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("targets an EOA, proxy or Safe"), "{err}");
    }

    #[tokio::test]
    async fn sign_order_as_rejects_a_plain_order_on_a_deposit_wallet_target() {
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let mut order = fixture_order();
        order.signature_type = SignatureType::Eoa;
        let target = SigningTarget::DepositWallet {
            wallet: DW,
            role: DepositWalletRole::Owner,
        };
        let err = sign_order_as(&order, &signer, 137, &target)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("signature type 3"), "{err}");
    }

    #[tokio::test]
    async fn sign_order_as_rejects_a_maker_that_is_not_the_target_wallet() {
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let mut order = fixture_order();
        order.maker = signer.address();
        order.signer = signer.address();
        let target = SigningTarget::DepositWallet {
            wallet: DW,
            role: DepositWalletRole::Owner,
        };
        let err = sign_order_as(&order, &signer, 137, &target)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("maker == signer =="), "{err}");
    }

    #[tokio::test]
    async fn sign_order_as_uses_the_neg_risk_exchange_in_digest_and_trailer() {
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let mut order = fixture_order();
        order.neg_risk = true;
        let target = SigningTarget::DepositWallet {
            wallet: DW,
            role: DepositWalletRole::Owner,
        };
        let sig = hex_bytes(&sign_order_as(&order, &signer, 137, &target).await.unwrap());
        let neg_risk_exchange = exchange_address(137, true).unwrap();
        let expected_sep = exchange_domain(137, neg_risk_exchange).eip712_hash_struct();
        assert_eq!(
            &sig[65..97],
            expected_sep.as_slice(),
            "trailer carries the neg-risk separator"
        );
        let digest = envelope_digest(&order, 137, neg_risk_exchange, DW).unwrap();
        let inner = alloy::primitives::Signature::from_raw(&sig[..65]).unwrap();
        assert_eq!(
            inner.recover_address_from_prehash(&digest).unwrap(),
            signer.address()
        );
    }

    #[tokio::test]
    async fn sign_order_as_delegates_plain_orders_to_sign_order() {
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let mut order = fixture_order();
        order.signature_type = SignatureType::Eoa;
        order.maker = signer.address();
        order.signer = signer.address();
        let via_target = sign_order_as(&order, &signer, 137, &SigningTarget::Eoa)
            .await
            .unwrap();
        let direct = sign_order(&order, &signer, 137).await.unwrap();
        assert_eq!(via_target, direct);
    }

    #[test]
    fn plain_digest_rejection_points_at_sign_order_as() {
        let err = compute_order_digest(&fixture_order(), 137)
            .unwrap_err()
            .to_string();
        assert!(err.contains("sign_order_as"), "{err}");
    }

    #[test]
    fn clob_auth_typed_data_is_the_upstream_json_shape() {
        let addr = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        let json = clob_auth_typed_data(addr, 137, 1700000000, 0);
        assert_eq!(
            json,
            serde_json::json!({
                "types": {
                    "EIP712Domain": [
                        { "name": "name", "type": "string" },
                        { "name": "version", "type": "string" },
                        { "name": "chainId", "type": "uint256" }
                    ],
                    "ClobAuth": [
                        { "name": "address", "type": "address" },
                        { "name": "timestamp", "type": "string" },
                        { "name": "nonce", "type": "uint256" },
                        { "name": "message", "type": "string" }
                    ]
                },
                "primaryType": "ClobAuth",
                "domain": { "name": "ClobAuthDomain", "version": "1", "chainId": 137 },
                "message": {
                    "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                    "timestamp": "1700000000",
                    "nonce": 0,
                    "message": "This message attests that I control the given wallet"
                }
            })
        );
    }
}
