use polyoxide_core::HttpClient;
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
    request::{AuthMode, Request},
};

/// Notifications namespace for notification operations
#[derive(Clone)]
pub struct Notifications {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl Notifications {
    fn l2_auth(&self) -> AuthMode {
        AuthMode::L2 {
            address: self.wallet.address(),
            credentials: self.credentials.clone(),
            signer: self.signer.clone(),
        }
    }

    /// List notifications for the current user
    pub fn list(&self) -> Request<Vec<Notification>> {
        Request::get(
            self.http_client.clone(),
            "/notifications",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Drop (dismiss) notifications by ID
    pub async fn drop(&self, ids: impl Into<Vec<String>>) -> Result<serde_json::Value, ClobError> {
        #[derive(Serialize)]
        struct Body {
            ids: Vec<String>,
        }

        Request::<serde_json::Value>::delete(
            self.http_client.clone(),
            "/notifications",
            self.l2_auth(),
            self.chain_id,
        )
        .body(&Body { ids: ids.into() })?
        .send()
        .await
    }
}

/// A notification from the CLOB API.
///
/// The API returns `{type: number, owner: string, payload: any}` — there is
/// no `id` field. Extra/unknown fields are captured in `extra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    #[serde(rename = "type")]
    pub notification_type: u32,
    pub owner: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_deserializes() {
        let json = r#"{
            "type": 1,
            "owner": "0xabc123",
            "payload": {"order_id": "order-456", "side": "BUY"}
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.notification_type, 1);
        assert_eq!(notif.owner, "0xabc123");
        assert_eq!(notif.payload["order_id"], "order-456");
    }

    #[test]
    fn notification_null_payload() {
        let json = r#"{
            "type": 0,
            "owner": "0xdef456",
            "payload": null
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.notification_type, 0);
        assert!(notif.payload.is_null());
    }

    #[test]
    fn notification_missing_payload() {
        let json = r#"{
            "type": 2,
            "owner": "0x789"
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.notification_type, 2);
        assert!(notif.payload.is_null());
    }
}
