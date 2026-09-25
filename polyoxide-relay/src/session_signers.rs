//! Session-signer authorization and revocation through the relayer
//! (`POST /v1/session-signers/authorizations` and `/revocations`).
//!
//! Neither route is in the published relayer OpenAPI; the contract is
//! `docs.polymarket.com/trading/session-keys` and Polymarket's official SDKs.
//! Both take a Deposit Wallet `Batch` signed by the owner (see
//! [`crate::deposit_wallet`]) plus the session-signer fields, with an
//! `Idempotency-Key` header.
//!
//! The two routes are gated differently, as in py-sdk: an authorization needs
//! Builder HMAC auth, while a revocation also accepts a relayer API key. Both
//! wait up to [`SESSION_SIGNER_REQUEST_TIMEOUT`] for the venue's answer.

use alloy::primitives::Address;
use polyoxide_core::SessionSignerScope;
use serde::{Deserialize, Serialize};

use crate::error::RelayError;
use crate::types::open_string_enum;

/// How long a session-signer `POST` waits for the venue's answer.
///
/// The venue validates, simulates, persists and broadcasts the batch before it
/// answers, so py-sdk gives these two routes a 300-second read timeout rather than
/// the client default. A slow success cut off early surfaces as a transport error,
/// and a caller that retries with a fresh idempotency key then submits a second
/// operation instead of replaying the first.
pub const SESSION_SIGNER_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Reject malformed scope lists before any I/O: empty, containing an empty scope,
/// duplicated, or `ALL` mixed with anything else.
///
/// The venue and py-sdk refuse an empty list, an empty scope and `ALL` alongside
/// other scopes. Duplicates are rejected here, client-side only. Compares on the
/// wire spelling, so `Other("ALL")` counts as `ALL`.
pub fn validate_scopes(scopes: &[SessionSignerScope]) -> Result<(), RelayError> {
    if scopes.is_empty() {
        return Err(RelayError::Api(
            "session-signer scopes need at least one entry".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for scope in scopes {
        if scope.as_str().is_empty() {
            return Err(RelayError::Api(
                "session-signer scope must not be empty".into(),
            ));
        }
        if !seen.insert(scope.as_str()) {
            return Err(RelayError::Api(format!(
                "duplicate session-signer scope {scope}"
            )));
        }
    }
    if scopes.iter().any(|s| s.as_str() == "ALL") && scopes.len() > 1 {
        return Err(RelayError::Api("scope ALL must be requested alone".into()));
    }
    Ok(())
}

open_string_enum! {
    /// Status of a session-signer authorization operation.
    SessionSignerAuthorizationStatus {
        /// Accepted; batch not yet broadcast.
        Submitted => "SUBMITTED",
        /// Broadcast; not yet in the session-signer registry.
        RegistryPending => "REGISTRY_PENDING",
        /// Live: the key appears in `GET /v1/user/session-signers`.
        Registered => "REGISTERED",
        /// Terminal failure.
        Failed => "FAILED",
        /// Terminal: a newer authorization for the same signer replaced this one.
        Superseded => "SUPERSEDED",
        /// Terminal: the venue needs manual intervention.
        RepairRequired => "REPAIR_REQUIRED",
    }
}

impl SessionSignerAuthorizationStatus {
    /// `Failed`, `Superseded` or `RepairRequired`.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::Superseded | Self::RepairRequired)
    }
}

open_string_enum! {
    /// Status of a session-signer revocation operation.
    SessionSignerRevocationStatus {
        /// Accepted.
        Pending => "PENDING",
        /// The key is fenced out of the registry; its open orders are being cancelled.
        Fenced => "FENCED",
        /// Open orders cancelled.
        Swept => "SWEPT",
        /// The on-chain revocation is broadcast.
        ChainSubmitted => "CHAIN_SUBMITTED",
        /// Confirmed on chain.
        Confirmed => "CONFIRMED",
        /// Terminal failure.
        Failed => "FAILED",
    }
}

impl SessionSignerRevocationStatus {
    /// Only `Failed`.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

/// Everything an authorization body needs except the owner's signature.
///
/// Produced by [`crate::RelayClient::authorize_session_signer_typed_data`] together
/// with the typed data to sign; pass it back with the signature to
/// [`crate::RelayClient::submit_session_signer_authorization`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSignerAuthorization {
    /// The Deposit Wallet.
    pub wallet_address: Address,
    /// The session key's EOA.
    pub session_signer_address: Address,
    /// Requested venues.
    pub scopes: Vec<SessionSignerScope>,
    /// Session-key expiry, Unix seconds.
    pub valid_until: u64,
    /// The wallet nonce the batch was built with.
    pub nonce: u64,
    /// The batch deadline, Unix seconds.
    pub deadline: u64,
}

/// Wire body of `POST /v1/session-signers/authorizations`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerAuthorizationBody {
    /// `deadline`: the batch deadline, Unix seconds as a decimal string.
    pub deadline: String,
    /// `nonce`: the wallet nonce the batch was signed with, as a decimal string.
    pub nonce: String,
    /// `scopes`: the requested venues, on their wire spelling.
    pub scopes: Vec<SessionSignerScope>,
    /// `sessionSignerAddress`: the session key's EOA, checksummed.
    pub session_signer_address: String,
    /// `signature`: the owner's signature over the batch, `0x`-prefixed hex.
    pub signature: String,
    /// `validUntil`: the session key's expiry, Unix seconds as a decimal string.
    pub valid_until: String,
    /// `walletAddress`: the Deposit Wallet, checksummed.
    pub wallet_address: String,
}

impl SessionSignerAuthorization {
    /// The wire body with `signature` filled in.
    pub fn body(&self, signature: &str) -> SessionSignerAuthorizationBody {
        SessionSignerAuthorizationBody {
            deadline: self.deadline.to_string(),
            nonce: self.nonce.to_string(),
            scopes: self.scopes.clone(),
            session_signer_address: self.session_signer_address.to_string(),
            signature: signature.to_string(),
            valid_until: self.valid_until.to_string(),
            wallet_address: self.wallet_address.to_string(),
        }
    }
}

/// Response of `POST /v1/session-signers/authorizations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerAuthorizationResponse {
    /// Opaque operation id.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// Where the authorization stands.
    pub status: SessionSignerAuthorizationStatus,
    /// Present once the batch is broadcast.
    #[serde(default)]
    pub transaction_hash: Option<String>,
    /// Poll this with [`crate::RelayClient::get_gasless_transaction`].
    pub transaction_id: String,
}

/// Everything a revocation body needs except the owner's signature.
///
/// Produced by [`crate::RelayClient::revoke_session_signer_typed_data`] together
/// with the typed data to sign; pass it back with the signature to
/// [`crate::RelayClient::submit_session_signer_revocation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSignerRevocation {
    /// The Deposit Wallet.
    pub wallet_address: Address,
    /// The session key's EOA.
    pub session_signer_address: Address,
    /// The wallet nonce the batch was built with.
    pub nonce: u64,
    /// The batch deadline, Unix seconds.
    pub deadline: u64,
}

/// Wire body of `POST /v1/session-signers/revocations`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerRevocationBody {
    /// `deadline`: the batch deadline, Unix seconds as a decimal string.
    pub deadline: String,
    /// `nonce`: the wallet nonce the batch was signed with, as a decimal string.
    pub nonce: String,
    /// `sessionSignerAddress`: the session key's EOA, checksummed.
    pub session_signer_address: String,
    /// `signature`: the owner's signature over the batch, `0x`-prefixed hex.
    pub signature: String,
    /// `walletAddress`: the Deposit Wallet, checksummed.
    pub wallet_address: String,
}

impl SessionSignerRevocation {
    /// The wire body with `signature` filled in.
    pub fn body(&self, signature: &str) -> SessionSignerRevocationBody {
        SessionSignerRevocationBody {
            deadline: self.deadline.to_string(),
            nonce: self.nonce.to_string(),
            session_signer_address: self.session_signer_address.to_string(),
            signature: signature.to_string(),
            wallet_address: self.wallet_address.to_string(),
        }
    }
}

/// Response of `POST /v1/session-signers/revocations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSignerRevocationResponse {
    /// Opaque operation id.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// Where the revocation stands.
    pub status: SessionSignerRevocationStatus,
    /// Whether the key was already fenced out of the registry when the relayer answered.
    pub fenced: bool,
    /// Poll this with [`crate::RelayClient::get_gasless_transaction`].
    pub transaction_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_core::SessionSignerScope;

    #[test]
    fn validate_scopes_accepts_the_documented_shapes() {
        assert!(validate_scopes(&[SessionSignerScope::Clob]).is_ok());
        assert!(
            validate_scopes(&[SessionSignerScope::Clob, SessionSignerScope::CombosRfq]).is_ok()
        );
        assert!(validate_scopes(&[SessionSignerScope::All]).is_ok());
    }

    #[test]
    fn validate_scopes_rejects_empty_duplicates_and_all_with_others() {
        assert!(validate_scopes(&[])
            .unwrap_err()
            .to_string()
            .contains("at least one"));
        assert!(
            validate_scopes(&[SessionSignerScope::Clob, SessionSignerScope::Clob])
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        assert!(
            validate_scopes(&[SessionSignerScope::All, SessionSignerScope::Clob])
                .unwrap_err()
                .to_string()
                .contains("ALL")
        );
        // Other("ALL") is compared on the wire spelling, so it cannot slip past.
        assert!(validate_scopes(&[
            SessionSignerScope::Other("ALL".into()),
            SessionSignerScope::Clob
        ])
        .is_err());
        assert!(validate_scopes(&[SessionSignerScope::Other(String::new())])
            .unwrap_err()
            .to_string()
            .contains("empty"));
    }

    #[test]
    fn session_signer_requests_wait_five_minutes_like_py_sdk() {
        assert_eq!(
            SESSION_SIGNER_REQUEST_TIMEOUT,
            std::time::Duration::from_secs(300)
        );
    }

    #[test]
    fn statuses_round_trip_and_flag_terminal_failures() {
        let s: SessionSignerAuthorizationStatus =
            serde_json::from_str("\"REGISTRY_PENDING\"").unwrap();
        assert_eq!(s, SessionSignerAuthorizationStatus::RegistryPending);
        assert!(!s.is_terminal_failure());
        for wire in ["FAILED", "SUPERSEDED", "REPAIR_REQUIRED"] {
            assert!(
                SessionSignerAuthorizationStatus::from_wire(wire).is_terminal_failure(),
                "{wire}"
            );
        }
        assert!(!SessionSignerAuthorizationStatus::Other("NEW_THING".into()).is_terminal_failure());

        let r: SessionSignerRevocationStatus = serde_json::from_str("\"FENCED\"").unwrap();
        assert_eq!(r, SessionSignerRevocationStatus::Fenced);
        assert!(SessionSignerRevocationStatus::Failed.is_terminal_failure());
        assert!(!SessionSignerRevocationStatus::Confirmed.is_terminal_failure());
    }

    #[test]
    fn responses_parse_the_sdk_shapes() {
        let a: SessionSignerAuthorizationResponse = serde_json::from_str(
            r#"{"operationId":"op-1","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-1"}"#,
        )
        .unwrap();
        assert_eq!(a.status, SessionSignerAuthorizationStatus::Submitted);
        assert_eq!(a.transaction_hash, None);
        assert_eq!(a.transaction_id, "tx-1");
        assert_eq!(a.operation_id.as_deref(), Some("op-1"));

        let r: SessionSignerRevocationResponse = serde_json::from_str(
            r#"{"operationId":"op-2","status":"FENCED","fenced":true,"transactionId":"tx-2"}"#,
        )
        .unwrap();
        assert_eq!(r.status, SessionSignerRevocationStatus::Fenced);
        assert!(r.fenced);
    }

    #[test]
    fn authorization_request_serialises_the_venue_body() {
        let req = SessionSignerAuthorization {
            wallet_address: "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50"
                .parse()
                .unwrap(),
            session_signer_address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
                .parse()
                .unwrap(),
            scopes: vec![SessionSignerScope::Clob],
            valid_until: 1815534000,
            nonce: 4,
            deadline: 1800000600,
        };
        let body = req.body("0xsig");
        let expected: serde_json::Value = serde_json::from_str(
            r#"{"deadline":"1800000600","nonce":"4","scopes":["CLOB"],"sessionSignerAddress":"0x70997970C51812dc3A010C7d01b50e0d17dc79C8","signature":"0xsig","validUntil":"1815534000","walletAddress":"0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50"}"#,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&body).unwrap(), expected);
    }
}
