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
        .body(&Body {
            ids: ids.into(),
        })?
        .send()
        .await
    }
}

/// A notification from the CLOB API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_deserializes_with_extra_fields() {
        let json = r#"{
            "id": "notif-123",
            "type": "order_filled",
            "message": "Your order was filled"
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.id, "notif-123");
        assert_eq!(notif.extra["type"], "order_filled");
    }
}
