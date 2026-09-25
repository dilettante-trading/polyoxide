use std::sync::Arc;

use alloy::{
    primitives::Address,
    signers::{local::PrivateKeySigner, Signer as AlloySigner},
};

use crate::error::ClobError;

/// The signer an [`Account`](crate::Account) holds: any `alloy` signer that
/// implements `sign_hash`, type-erased.
///
/// Local keys and KMS-backed signers (AWS, GCP) qualify, so the process never
/// has to hold raw key material to trade. Hardware wallets such as Ledger and
/// Trezor refuse raw-hash signing and are not usable here; a hardware-held
/// owner key instead signs the L1 auth typed data out of process (via
/// [`crate::clob_auth_typed_data`] and `Clob::derive_api_key_with_signature`)
/// and authorizes a session key that implements `sign_hash` for trading.
pub type DynSigner = dyn AlloySigner + Send + Sync;

/// The EIP-712 signing half of an account.
///
/// Holds an address and, unless built with [`Wallet::l2_only`], a signer for it.
/// An L2-only wallet can authenticate HMAC (L2) requests, which only need the
/// address, but cannot sign orders or the L1 auth message.
#[derive(Clone)]
pub struct Wallet {
    signer: Option<Arc<DynSigner>>,
    address: Address,
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address)
            .field("signer", &self.signer.is_some())
            .finish()
    }
}

impl Wallet {
    /// Create a wallet from a hex-encoded private key (with or without `0x`).
    pub fn from_private_key(private_key: &str) -> Result<Self, ClobError> {
        let signer = private_key
            .parse::<PrivateKeySigner>()
            .map_err(|e| ClobError::Crypto(format!("Failed to parse private key: {}", e)))?;
        Ok(Self::from_signer(signer))
    }

    /// Create a wallet around any `alloy` signer.
    ///
    /// The signer must implement `sign_hash`; hardware wallets such as Ledger
    /// and Trezor do not and are not usable here.
    pub fn from_signer<S>(signer: S) -> Self
    where
        S: AlloySigner + Send + Sync + 'static,
    {
        Self {
            address: signer.address(),
            signer: Some(Arc::new(signer)),
        }
    }

    /// Create a wallet that knows its address but holds no key.
    ///
    /// Enough for every L2 (HMAC) request: reads, cancels, posting an already
    /// signed order. [`Wallet::signer`] returns an error.
    pub fn l2_only(address: Address) -> Self {
        Self {
            signer: None,
            address,
        }
    }

    /// The wallet address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Whether this wallet can sign.
    pub fn has_signer(&self) -> bool {
        self.signer.is_some()
    }

    /// The signer, or a validation error for an L2-only wallet.
    pub fn signer(&self) -> Result<&DynSigner, ClobError> {
        self.signer.as_deref().ok_or_else(|| {
            ClobError::validation(
                "this account is L2-only (no signing key): it can read, cancel and post \
                 signed orders, but cannot sign orders or the L1 auth message",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    // Well-known test private key (DO NOT use in production)
    const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const TEST_ADDR: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    #[test]
    fn test_wallet_debug_shows_address_not_key() {
        let wallet = Wallet::from_private_key(TEST_KEY).unwrap();
        let debug_output = format!("{:?}", wallet);
        assert!(debug_output.contains("address"), "{debug_output}");
        assert!(
            !debug_output
                .contains("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            "Debug should NOT contain private key: {debug_output}"
        );
    }

    #[test]
    fn from_private_key_has_a_signer_at_the_derived_address() {
        let wallet = Wallet::from_private_key(TEST_KEY).unwrap();
        assert_eq!(wallet.address(), TEST_ADDR);
        assert!(wallet.has_signer());
        assert_eq!(wallet.signer().unwrap().address(), TEST_ADDR);
    }

    #[test]
    fn from_signer_accepts_any_alloy_signer() {
        let local: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let wallet = Wallet::from_signer(local);
        assert_eq!(wallet.address(), TEST_ADDR);
        assert!(wallet.has_signer());
    }

    #[test]
    fn l2_only_wallet_has_an_address_but_refuses_to_sign() {
        let wallet = Wallet::l2_only(TEST_ADDR);
        assert_eq!(wallet.address(), TEST_ADDR);
        assert!(!wallet.has_signer());
        let err = wallet.signer().map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("L2-only"), "{err}");
        assert!(
            format!("{wallet:?}").contains("signer: false"),
            "{wallet:?}"
        );
    }

    #[tokio::test]
    async fn erased_signer_signs_and_recovers_to_the_wallet_address() {
        use alloy::primitives::B256;
        let wallet = Wallet::from_private_key(TEST_KEY).unwrap();
        let digest = B256::repeat_byte(0x42);
        let signature = wallet.signer().unwrap().sign_hash(&digest).await.unwrap();
        let recovered = signature.recover_address_from_prehash(&digest).unwrap();
        assert_eq!(recovered, TEST_ADDR);
    }
}
