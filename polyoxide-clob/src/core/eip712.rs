use alloy::{
    primitives::{keccak256, U256},
    signers::Signer as AlloySigner,
    sol,
    sol_types::{Eip712Domain, SolStruct},
};

use crate::{
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

/// Convert a CLOB order to the EIP-712 protocol struct for hashing/signing.
fn order_to_protocol(order: &ClobOrder) -> Result<protocol::Order, ClobError> {
    // Authoritative guard at the lowest signing choke point: every signing path
    // (`Clob::sign_order`, `Account::sign_order`, and the create_* convenience methods)
    // funnels through here, so this closes the hand-built-order bypass. The V2 contract
    // cannot validate a `signatureType: 3` order as an ECDSA signature.
    if order.signature_type == SignatureType::Poly1271 {
        return Err(ClobError::Crypto(
            "Poly1271 (EIP-1271) signing is not yet supported; use EOA/PolyProxy/PolyGnosisSafe"
                .into(),
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

/// Compute the EIP-712 digest for an order (without signing).
fn compute_order_digest(
    order: &ClobOrder,
    chain_id: u64,
) -> Result<alloy::primitives::B256, ClobError> {
    let chain = Chain::from_chain_id(chain_id)
        .ok_or_else(|| ClobError::Crypto(format!("Unsupported chain ID: {}", chain_id)))?;
    let contracts = chain.contracts();

    let verifying_contract = if order.neg_risk {
        contracts.neg_risk_exchange
    } else {
        contracts.exchange
    };

    let domain = protocol::EIP712Domain {
        name: "Polymarket CTF Exchange".to_string(),
        version: "2".to_string(),
        chainId: U256::from(chain_id),
        verifyingContract: verifying_contract,
    };

    let order_struct = order_to_protocol(order)?;
    let struct_hash = order_struct.eip712_hash_struct();
    let domain_separator = domain.eip712_hash_struct();

    let mut message = Vec::new();
    message.extend_from_slice(b"\x19\x01");
    message.extend_from_slice(domain_separator.as_slice());
    message.extend_from_slice(struct_hash.as_slice());
    Ok(keccak256(&message))
}

/// Sign an order with EIP-712
pub async fn sign_order<S: AlloySigner>(
    order: &ClobOrder,
    signer: &S,
    chain_id: u64,
) -> Result<String, ClobError> {
    let digest = compute_order_digest(order, chain_id)?;
    let signature = signer.sign_hash(&digest).await?;
    Ok(format!("0x{}", hex::encode(signature.as_bytes())))
}

/// Sign CLOB auth message for API key creation
pub async fn sign_clob_auth<S: AlloySigner>(
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
    let mut digest_message = Vec::new();
    digest_message.extend_from_slice(b"\x19\x01");
    digest_message.extend_from_slice(domain_separator.as_slice());
    digest_message.extend_from_slice(struct_hash.as_slice());
    let digest = keccak256(&digest_message);

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
    fn order_to_protocol_rejects_poly1271() {
        // Hand-built order with Poly1271 must be rejected at the signing choke point,
        // independent of the higher-level create_order/create_market_order guards.
        let mut order = make_test_order(false);
        order.signature_type = SignatureType::Poly1271;
        let err = order_to_protocol(&order).unwrap_err();
        assert!(
            err.to_string().contains("Poly1271"),
            "expected a Poly1271 rejection, got: {err}"
        );
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
            "signing must reject Poly1271 (EIP-1271 not yet supported)"
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
