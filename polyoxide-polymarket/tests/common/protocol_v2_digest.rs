//! An order's digest under a given exchange domain separator, built from the
//! order's fields so a test does not lean on the crate's own hashing. Shared by
//! `mock_clob.rs` and `trading_clob.rs`.

use alloy::primitives::{keccak256, Address, Signature, B256, U256};
use alloy::sol_types::SolStruct;
use polyoxide_polymarket::clob::{Order as ClobOrder, OrderSide};
use std::str::FromStr;

alloy::sol! {
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
}

pub fn digest(domain_separator: B256, order: &ClobOrder) -> B256 {
    let number = |text: &str| U256::from_str_radix(text, 10).unwrap();
    let typed = Order {
        salt: number(&order.salt),
        maker: order.maker,
        signer: order.signer,
        tokenId: number(&order.token_id),
        makerAmount: number(&order.maker_amount),
        takerAmount: number(&order.taker_amount),
        side: match order.side {
            OrderSide::Buy => 0,
            OrderSide::Sell => 1,
        },
        signatureType: order.signature_type as u8,
        timestamp: number(&order.timestamp),
        metadata: order.metadata,
        builder: order.builder,
    };
    let mut message = Vec::with_capacity(66);
    message.extend_from_slice(b"\x19\x01");
    message.extend_from_slice(domain_separator.as_slice());
    message.extend_from_slice(typed.eip712_hash_struct().as_slice());
    keccak256(&message)
}

/// The address that signed `digest`, recovered from a `0x` signature.
pub fn signer_of(signature: &str, digest: B256) -> Address {
    Signature::from_str(signature)
        .unwrap()
        .recover_address_from_prehash(&digest)
        .unwrap()
}
