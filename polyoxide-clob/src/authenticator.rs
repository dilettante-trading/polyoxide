//! The CLOB's three ways of signing a request, as core's [`Authenticator`]s.
//!
//! Each runs on every attempt of [`HttpClient::send`](polyoxide_core::HttpClient::send),
//! so a retried request carries a fresh timestamp and signature. A signing
//! failure is boxed into [`ApiError::Sign`], which [`ClobError`]'s
//! `From<ApiError>` takes back out, so the caller sees the clob error the
//! signer raised.

use alloy::primitives::Address;
use polyoxide_core::{current_timestamp, ApiError, Authenticator, RequestParts};
use polyoxide_venue::Secret;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::{
    account::{Credentials, Signer, Wallet},
    core::eip712::sign_clob_auth,
    error::ClobError,
};

const POLY_ADDRESS: HeaderName = HeaderName::from_static("poly_address");
const POLY_SIGNATURE: HeaderName = HeaderName::from_static("poly_signature");
const POLY_TIMESTAMP: HeaderName = HeaderName::from_static("poly_timestamp");
const POLY_NONCE: HeaderName = HeaderName::from_static("poly_nonce");
const POLY_API_KEY: HeaderName = HeaderName::from_static("poly_api_key");
const POLY_PASSPHRASE: HeaderName = HeaderName::from_static("poly_passphrase");

/// A clob error raised while signing, carried through core's loop.
fn sign_error(err: ClobError) -> ApiError {
    ApiError::Sign(Box::new(err))
}

/// A header value, or the signing error a value that cannot be one is.
fn value(text: &str) -> Result<HeaderValue, ApiError> {
    HeaderValue::from_str(text).map_err(|err| ApiError::Sign(Box::new(err)))
}

/// L2: HMAC-SHA256 over `timestamp + method + path [+ body]` with the API
/// secret, plus the API key and passphrase.
#[derive(Debug)]
pub(crate) struct L2Auth {
    pub(crate) address: Address,
    pub(crate) credentials: Secret<Credentials>,
    pub(crate) signer: Signer,
}

impl Authenticator for L2Auth {
    async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        let timestamp = current_timestamp();
        let message = Signer::create_message(
            timestamp,
            parts.method.as_str(),
            &parts.path,
            parts.body.as_deref(),
        );
        let signature = self.signer.sign(&message).map_err(sign_error)?;
        let credentials = self.credentials.expose();

        let headers = &mut parts.headers;
        // The L2 address is sent lowercase (`{:?}`), as it always has been:
        // the server accepts it, and nothing shows it is case-sensitive here.
        headers.insert(POLY_ADDRESS, value(&format!("{:?}", self.address))?);
        headers.insert(POLY_SIGNATURE, value(&signature)?);
        headers.insert(POLY_TIMESTAMP, value(&timestamp.to_string())?);
        headers.insert(POLY_API_KEY, value(&credentials.key)?);
        headers.insert(POLY_PASSPHRASE, value(&credentials.passphrase)?);
        Ok(())
    }
}

/// L1: the EIP-712 `ClobAuth` message, signed by the wallet's key with a fresh
/// timestamp each attempt.
#[derive(Debug)]
pub(crate) struct L1Auth {
    pub(crate) wallet: Wallet,
    pub(crate) nonce: u32,
    pub(crate) chain_id: u64,
}

impl Authenticator for L1Auth {
    async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        let timestamp = current_timestamp();
        let signer = self.wallet.signer().map_err(sign_error)?;
        let signature = sign_clob_auth(signer, self.chain_id, timestamp, self.nonce)
            .await
            .map_err(sign_error)?;
        l1_headers(
            &mut parts.headers,
            self.wallet.address(),
            &signature,
            timestamp,
            self.nonce,
        )
    }
}

/// L1 with a signature produced outside this process: the same four headers
/// as [`L1Auth`], resent unchanged on every attempt, since only the external
/// wallet can sign a new timestamp.
pub(crate) struct L1Signed {
    pub(crate) address: Address,
    pub(crate) nonce: u32,
    pub(crate) timestamp: u64,
    pub(crate) signature: String,
}

/// Written by hand so the signature is never printed.
impl std::fmt::Debug for L1Signed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("L1Signed")
            .field("address", &self.address)
            .field("nonce", &self.nonce)
            .field("timestamp", &self.timestamp)
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

impl Authenticator for L1Signed {
    async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        l1_headers(
            &mut parts.headers,
            self.address,
            &self.signature,
            self.timestamp,
            self.nonce,
        )
    }
}

/// Set the four `POLY_*` headers L1 auth sends, shared by [`L1Auth`] (which signs
/// first) and [`L1Signed`] (which already has a signature).
fn l1_headers(
    headers: &mut HeaderMap,
    address: Address,
    signature: &str,
    timestamp: u64,
    nonce: u32,
) -> Result<(), ApiError> {
    // EIP-55 checksummed, matching py-clob-client, which sends
    // eth_account's `signer.address()`. Note `Display`/`to_string`
    // checksums but `{:?}` lowercases — the L1 path is where this
    // can matter, since the server recovers the address from the
    // signature and compares it against this header.
    //
    // The L2 authenticator deliberately still uses `{:?}`: it sends
    // lowercase today and works, so there is no evidence the server
    // is case-sensitive there and no reason to churn a working path.
    // Don't "unify" these without a live check on both.
    headers.insert(POLY_ADDRESS, value(&address.to_string())?);
    headers.insert(POLY_SIGNATURE, value(signature)?);
    headers.insert(POLY_TIMESTAMP, value(&timestamp.to_string())?);
    headers.insert(POLY_NONCE, value(&nonce.to_string())?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::local::PrivateKeySigner;
    use reqwest::Method;

    // Anvil/Hardhat account #0.
    const ANVIL_KEY_0: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    /// Proves the signature-in path is not just parallel code that happens to work, but
    /// produces the exact same wire headers as the signer-based path given the same
    /// signature — at the header level, independent of the mock-server assertions in
    /// `tests/mock_api.rs`.
    #[tokio::test]
    async fn l1_and_l1_signed_produce_identical_headers() {
        let wallet = Wallet::from_private_key(ANVIL_KEY_0).unwrap();
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();

        let auth_l1 = L1Auth {
            wallet: wallet.clone(),
            nonce: 7,
            chain_id: 137,
        };
        let mut request = RequestParts::new(Method::GET, "/x");
        auth_l1.sign(&mut request, 0).await.unwrap();
        let timestamp: u64 = request
            .headers
            .get("POLY_TIMESTAMP")
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();

        let signature = sign_clob_auth(&signer, 137, timestamp, 7).await.unwrap();
        let auth_signed = L1Signed {
            address: wallet.address(),
            nonce: 7,
            timestamp,
            signature,
        };
        let mut request2 = RequestParts::new(Method::GET, "/x");
        auth_signed.sign(&mut request2, 0).await.unwrap();

        for name in [
            "POLY_ADDRESS",
            "POLY_SIGNATURE",
            "POLY_TIMESTAMP",
            "POLY_NONCE",
        ] {
            assert_eq!(
                request.headers.get(name),
                request2.headers.get(name),
                "{name} differs"
            );
        }
    }

    #[test]
    fn l1_signed_debug_redacts_the_signature() {
        let auth = L1Signed {
            address: Address::ZERO,
            nonce: 1,
            timestamp: 1700000000,
            signature: "0xdeadbeefdeadbeef".to_string(),
        };
        let debug = format!("{auth:?}");
        assert!(!debug.contains("deadbeef"), "{debug}");
        assert!(debug.contains("REDACTED"), "{debug}");
    }
}
