//! The routes a read-only API key may call.

use alloy::primitives::Address;
use polyoxide_core::{DynAuthenticator, HttpClient};
use polyoxide_venue::Secret;

use crate::clob::{api::orders::WalletOpenOrders, authenticator::ReadonlyAuth};

/// Read-only API key namespace, from [`Clob::readonly`](crate::clob::Clob::readonly).
///
/// Each request is sent with `POLY_READONLY_API_KEY` and `POLY_ADDRESS` alone,
/// with no L2 signature, so it needs no [`Account`](crate::clob::Account) and
/// works on [`Clob::public`](crate::clob::Clob::public). A key is minted with
/// [`Auth::create_readonly_key`](crate::clob::api::Auth::create_readonly_key).
/// Its `Debug` never prints the key.
#[derive(Debug, Clone)]
pub struct Readonly {
    pub(crate) http_client: HttpClient,
    pub(crate) key: Secret<String>,
    pub(crate) address: Address,
}

impl Readonly {
    /// List every live order on the Deposit Wallet of the key's address
    /// (`GET /data/wallet-open-orders`), whichever signer or API key placed it.
    ///
    /// The same route and builder as
    /// [`Orders::wallet_open_orders`](crate::clob::api::Orders::wallet_open_orders),
    /// whose notes on paging and on which addresses may read it apply here: the
    /// address must be the wallet's owner or one of its non-revoked session
    /// signers.
    pub fn wallet_open_orders(&self) -> WalletOpenOrders {
        WalletOpenOrders::new(
            self.http_client.clone(),
            DynAuthenticator::new_arc(ReadonlyAuth {
                key: self.key.clone(),
                address: self.address,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_namespace_debug_never_prints_the_key() {
        let readonly =
            crate::clob::Clob::public().readonly("readonly-key-7f3a9c", Address::repeat_byte(0x11));
        let debug = format!("{readonly:?}");
        assert!(!debug.contains("readonly-key-7f3a9c"), "{debug}");
        assert!(debug.contains("[REDACTED]"), "{debug}");
        assert!(
            debug.contains("0x1111111111111111111111111111111111111111"),
            "{debug}"
        );
    }
}
