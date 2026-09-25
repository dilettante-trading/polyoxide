//! What an [`crate::Account`] signs for.
//!
//! The signing EOA and the on-chain account that an order is made for are not
//! always the same address. A Polymarket proxy or Gnosis Safe has a separate
//! funder, and a Deposit Wallet is a smart account that both the owner's key and
//! an authorized session key sign for. The role cannot be inferred from
//! addresses, since both the owner and a session key are EOAs distinct from the
//! wallet, so it is stated here explicitly.

use alloy::primitives::Address;

use crate::types::SignatureType;

/// Whether the signing key is the Deposit Wallet's owner or an authorized session key.
///
/// A session key's order signatures are wrapped in an extra ERC-6492-style
/// envelope naming the session signer; an owner's are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositWalletRole {
    /// The EOA that owns the Deposit Wallet; the wallet's address is derived from this key.
    Owner,
    /// A key the owner authorized through `authorizeSessionSigner`; it can trade but not withdraw.
    SessionKey,
}

/// The on-chain account an [`crate::Account`] signs orders for.
///
/// Defaults to [`SigningTarget::Eoa`], which is the behaviour every existing
/// caller had: the signing key is the maker. Set another variant with
/// `Account::with_target`. A per-call `funder` on order parameters still sets
/// the maker; a per-call `signature_type` must agree with a Deposit Wallet
/// target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SigningTarget {
    /// The signing key is the maker (`signatureType` 0).
    #[default]
    Eoa,
    /// A Polymarket proxy wallet funds the orders (`signatureType` 1).
    PolyProxy {
        /// The proxy wallet address.
        funder: Address,
    },
    /// A Gnosis Safe funds the orders (`signatureType` 2).
    PolyGnosisSafe {
        /// The Safe address.
        funder: Address,
    },
    /// A Deposit Wallet is both maker and signer (`signatureType` 3); the key
    /// signs an ERC-7739 envelope on its behalf.
    DepositWallet {
        /// The Deposit Wallet address.
        wallet: Address,
        /// Owner key or session key.
        role: DepositWalletRole,
    },
}

impl SigningTarget {
    /// The `signatureType` this target signs with.
    pub fn signature_type(&self) -> SignatureType {
        match self {
            Self::Eoa => SignatureType::Eoa,
            Self::PolyProxy { .. } => SignatureType::PolyProxy,
            Self::PolyGnosisSafe { .. } => SignatureType::PolyGnosisSafe,
            Self::DepositWallet { .. } => SignatureType::Poly1271,
        }
    }

    /// The order `maker` for a key at `eoa`.
    pub fn maker(&self, eoa: Address) -> Address {
        match self {
            Self::Eoa => eoa,
            Self::PolyProxy { funder } | Self::PolyGnosisSafe { funder } => *funder,
            Self::DepositWallet { wallet, .. } => *wallet,
        }
    }

    /// The order `signer` field for a key at `eoa`.
    ///
    /// Only a Deposit Wallet puts something other than the EOA here: the venue
    /// requires `maker == signer == wallet` for `signatureType` 3.
    pub fn order_signer(&self, eoa: Address) -> Address {
        match self {
            Self::DepositWallet { wallet, .. } => *wallet,
            _ => eoa,
        }
    }

    /// The Deposit Wallet and role, when this target is one.
    pub fn deposit_wallet(&self) -> Option<(Address, DepositWalletRole)> {
        match self {
            Self::DepositWallet { wallet, role } => Some((*wallet, *role)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    const EOA: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    const OTHER: Address = address!("57ffbc34de23124faeb8387fcd689d314e57accd");

    #[test]
    fn eoa_target_signs_as_itself() {
        let t = SigningTarget::Eoa;
        assert_eq!(t.signature_type(), SignatureType::Eoa);
        assert_eq!(t.maker(EOA), EOA);
        assert_eq!(t.order_signer(EOA), EOA);
        assert_eq!(t.deposit_wallet(), None);
    }

    #[test]
    fn proxy_targets_put_the_funder_as_maker_and_the_eoa_as_signer() {
        let proxy = SigningTarget::PolyProxy { funder: OTHER };
        assert_eq!(proxy.signature_type(), SignatureType::PolyProxy);
        assert_eq!(proxy.maker(EOA), OTHER);
        assert_eq!(proxy.order_signer(EOA), EOA);

        let safe = SigningTarget::PolyGnosisSafe { funder: OTHER };
        assert_eq!(safe.signature_type(), SignatureType::PolyGnosisSafe);
        assert_eq!(safe.maker(EOA), OTHER);
        assert_eq!(safe.order_signer(EOA), EOA);
    }

    #[test]
    fn deposit_wallet_target_is_both_maker_and_signer() {
        for role in [DepositWalletRole::Owner, DepositWalletRole::SessionKey] {
            let t = SigningTarget::DepositWallet {
                wallet: OTHER,
                role,
            };
            assert_eq!(t.signature_type(), SignatureType::Poly1271);
            assert_eq!(t.maker(EOA), OTHER);
            assert_eq!(t.order_signer(EOA), OTHER);
            assert_eq!(t.deposit_wallet(), Some((OTHER, role)));
        }
    }

    #[test]
    fn default_target_is_eoa() {
        assert_eq!(SigningTarget::default(), SigningTarget::Eoa);
    }
}
