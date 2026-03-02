use alloy::{
    primitives::{keccak256, Address, U256},
    signers::Signer as AlloySigner,
    sol,
    sol_types::SolStruct,
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
            address taker;
            uint256 tokenId;
            uint256 makerAmount;
            uint256 takerAmount;
            uint256 expiration;
            uint256 nonce;
            uint256 feeRateBps;
            uint8 side;
            uint8 signatureType;
        }

        #[derive(Debug, PartialEq, Eq)]
        struct ClobAuth {
            string message;
        }
    }
}

/// Convert a CLOB order to the EIP-712 protocol struct for hashing/signing.
fn order_to_protocol(order: &ClobOrder) -> Result<protocol::Order, ClobError> {
    Ok(protocol::Order {
        salt: U256::from_str_radix(&order.salt, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid salt: {}", e)))?,
        maker: order.maker,
        signer: order.signer,
        taker: order.taker,
        tokenId: U256::from_str_radix(&order.token_id, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid token_id: {}", e)))?,
        makerAmount: U256::from_str_radix(&order.maker_amount, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid maker_amount: {}", e)))?,
        takerAmount: U256::from_str_radix(&order.taker_amount, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid taker_amount: {}", e)))?,
        expiration: U256::from_str_radix(&order.expiration, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid expiration: {}", e)))?,
        nonce: U256::from_str_radix(&order.nonce, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid nonce: {}", e)))?,
        feeRateBps: U256::from_str_radix(&order.fee_rate_bps, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid fee_rate_bps: {}", e)))?,
        side: match order.side {
            crate::types::OrderSide::Buy => 0,
            crate::types::OrderSide::Sell => 1,
        },
        signatureType: match order.signature_type {
            SignatureType::Eoa => 0,
            SignatureType::PolyProxy => 1,
            SignatureType::PolyGnosisSafe => 2,
        },
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
        version: "1".to_string(),
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
    let domain = protocol::EIP712Domain {
        name: "ClobAuthDomain".to_string(),
        version: "1".to_string(),
        chainId: U256::from(chain_id),
        verifyingContract: Address::ZERO,
    };

    let message = format!(
        "This message attests that I control the given wallet\ntimestamp: {}\nnonce: {}",
        timestamp, nonce
    );

    let clob_auth = protocol::ClobAuth { message };

    // Compute struct hash and domain separator
    let struct_hash = clob_auth.eip712_hash_struct();
    let domain_separator = domain.eip712_hash_struct();

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
    use alloy::primitives::address;
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
            taker: Address::ZERO,
            token_id: "100".to_string(),
            maker_amount: "5000000".to_string(),
            taker_amount: "10000000".to_string(),
            expiration: "0".to_string(),
            nonce: "0".to_string(),
            fee_rate_bps: "100".to_string(),
            side: OrderSide::Buy,
            signature_type: SignatureType::Eoa,
            neg_risk,
        }
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
        assert_eq!(proto.taker, Address::ZERO);
        assert_eq!(proto.tokenId, U256::from(100u64));
        assert_eq!(proto.makerAmount, U256::from(5000000u64));
        assert_eq!(proto.takerAmount, U256::from(10000000u64));
        assert_eq!(proto.expiration, U256::ZERO);
        assert_eq!(proto.nonce, U256::ZERO);
        assert_eq!(proto.feeRateBps, U256::from(100u64));
        assert_eq!(proto.side, 0); // Buy
        assert_eq!(proto.signatureType, 0); // Eoa
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
            taker: Address::ZERO,
            tokenId: U256::from(100u64),
            makerAmount: U256::from(1000u64),
            takerAmount: U256::from(2000u64),
            expiration: U256::ZERO,
            nonce: U256::ZERO,
            feeRateBps: U256::from(100u64),
            side: 0,
            signatureType: 0,
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
            taker: Address::ZERO,
            tokenId: U256::from(100u64),
            makerAmount: U256::from(1000u64),
            takerAmount: U256::from(2000u64),
            expiration: U256::ZERO,
            nonce: U256::ZERO,
            feeRateBps: U256::from(100u64),
            side: 0,
            signatureType: 0,
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
            taker: Address::ZERO,
            tokenId: U256::ZERO,
            makerAmount: U256::ZERO,
            takerAmount: U256::ZERO,
            expiration: U256::ZERO,
            nonce: U256::ZERO,
            feeRateBps: U256::ZERO,
            side: 0,
            signatureType: 0,
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
