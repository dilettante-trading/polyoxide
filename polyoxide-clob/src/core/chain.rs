use alloy::primitives::{address, Address};

/// Supported Polygon chains for CLOB order signing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    /// Polygon mainnet (chain ID 137).
    PolygonMainnet,
    /// Polygon Amoy testnet (chain ID 80002).
    PolygonAmoy,
}

impl Chain {
    /// Get the chain ID
    pub const fn chain_id(&self) -> u64 {
        match self {
            Chain::PolygonMainnet => 137,
            Chain::PolygonAmoy => 80002,
        }
    }

    /// Get contracts for this chain
    pub const fn contracts(&self) -> Contracts {
        match self {
            Chain::PolygonMainnet => Contracts::POLYGON_MAINNET,
            Chain::PolygonAmoy => Contracts::POLYGON_AMOY,
        }
    }

    /// Create Chain from chain ID
    pub const fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            137 => Some(Self::PolygonMainnet),
            80002 => Some(Self::PolygonAmoy),
            _ => None,
        }
    }
}

/// Contract addresses for different chains
#[derive(Debug, Clone, Copy)]
pub struct Contracts {
    pub exchange: Address,
    pub neg_risk_exchange: Address,
    pub neg_risk_adapter: Address,
    pub collateral: Address,
    pub conditional_tokens: Address,
}

impl Contracts {
    /// Polygon mainnet contracts (chain ID 137)
    pub const POLYGON_MAINNET: Self = Self {
        exchange: address!("4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"),
        neg_risk_exchange: address!("C5d563A36AE78145C45a50134d48A1215220f80a"),
        neg_risk_adapter: address!("d91E80cF2E7be2e162c6513ceD06f1dD0dA35296"),
        collateral: address!("2791Bca1f2de4661ED88A30C99A7a9449Aa84174"),
        conditional_tokens: address!("4D97DCd97eC945f40cF65F87097ACe5EA0476045"),
    };

    /// Polygon Amoy testnet contracts (chain ID 80002)
    pub const POLYGON_AMOY: Self = Self {
        exchange: address!("dFE02Eb6733538f8Ea35D585af8DE5958AD99E40"),
        neg_risk_exchange: address!("d91E80cF2E7be2e162c6513ceD06f1dD0dA35296"),
        neg_risk_adapter: address!("d0D0E471E88e0A8E7C304F2df3A0Cc7400fe4635"),
        collateral: address!("9c4e1703476e875070ee25b56a58b008cfb8fa78"),
        conditional_tokens: address!("69308FB512518e39F9b16112fA8d994F4e2Bf8bB"),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id() {
        assert_eq!(Chain::PolygonMainnet.chain_id(), 137);
        assert_eq!(Chain::PolygonAmoy.chain_id(), 80002);
    }

    #[test]
    fn test_from_chain_id() {
        assert_eq!(Chain::from_chain_id(137), Some(Chain::PolygonMainnet));
        assert_eq!(Chain::from_chain_id(80002), Some(Chain::PolygonAmoy));
        assert_eq!(Chain::from_chain_id(1), None);
        assert_eq!(Chain::from_chain_id(999), None);
    }

    #[test]
    fn test_chain_contracts() {
        let mainnet_contracts = Chain::PolygonMainnet.contracts();
        assert_eq!(
            mainnet_contracts.exchange,
            Contracts::POLYGON_MAINNET.exchange
        );

        let amoy_contracts = Chain::PolygonAmoy.contracts();
        assert_eq!(amoy_contracts.exchange, Contracts::POLYGON_AMOY.exchange);
    }

    #[test]
    fn test_polygon_mainnet_addresses() {
        let contracts = Contracts::POLYGON_MAINNET;

        // Verify addresses are not zero
        assert_ne!(contracts.exchange, Address::ZERO);
        assert_ne!(contracts.neg_risk_exchange, Address::ZERO);
        assert_ne!(contracts.neg_risk_adapter, Address::ZERO);
        assert_ne!(contracts.collateral, Address::ZERO);
        assert_ne!(contracts.conditional_tokens, Address::ZERO);
    }

    #[test]
    fn test_polygon_amoy_addresses() {
        let contracts = Contracts::POLYGON_AMOY;

        // Verify addresses are not zero
        assert_ne!(contracts.exchange, Address::ZERO);
        assert_ne!(contracts.neg_risk_exchange, Address::ZERO);
        assert_ne!(contracts.neg_risk_adapter, Address::ZERO);
        assert_ne!(contracts.collateral, Address::ZERO);
        assert_ne!(contracts.conditional_tokens, Address::ZERO);
    }

    #[test]
    fn test_chain_is_copy() {
        let chain = Chain::PolygonMainnet;
        let _copy1 = chain;
        let _copy2 = chain; // Should still work
    }

    #[test]
    fn test_chain_equality() {
        assert_eq!(Chain::PolygonMainnet, Chain::PolygonMainnet);
        assert_eq!(Chain::PolygonAmoy, Chain::PolygonAmoy);
        assert_ne!(Chain::PolygonMainnet, Chain::PolygonAmoy);
    }
}
