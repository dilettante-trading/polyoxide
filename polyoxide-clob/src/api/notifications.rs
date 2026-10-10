use std::sync::Arc;

use polyoxide_core::reqwest::Method;
use polyoxide_core::{DynAuthenticator, HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{error::ClobError, types::SignatureType};

/// Notifications namespace for notification operations
#[derive(Clone)]
pub struct Notifications {
    pub(crate) http_client: HttpClient,
    pub(crate) l2: Arc<DynAuthenticator<'static>>,
    pub(crate) signature_type: SignatureType,
}

impl Notifications {
    /// List notifications for the current user.
    ///
    /// The CLOB API requires a `signature_type` query parameter to derive the
    /// account address; it is taken from the client configuration.
    pub fn list(&self) -> Request<Vec<Notification>, ClobError> {
        Request::new(self.http_client.clone(), "/notifications")
            .authenticator(self.l2.clone())
            .query("signature_type", self.signature_type as u8)
    }

    /// Drop (dismiss) notifications by ID.
    ///
    /// Takes the numeric [`Notification::id`] values.
    ///
    /// **Unverified against the wire.** Upstream documents this as
    /// `DELETE /notifications?ids=1,2,3` — a comma-separated *query parameter*
    /// — while this sends a JSON body. That mismatch has not been confirmed
    /// live, because doing so marks real notifications as read; it is flagged
    /// rather than fixed. This method has no test coverage.
    pub async fn drop(&self, ids: impl Into<Vec<u64>>) -> Result<serde_json::Value, ClobError> {
        #[derive(Serialize)]
        struct Body {
            ids: Vec<u64>,
        }

        Request::<serde_json::Value, ClobError>::new(self.http_client.clone(), "/notifications")
            .method(Method::DELETE)
            .authenticator(self.l2.clone())
            .body(&Body { ids: ids.into() })?
            .send()
            .await
    }
}

/// A notification from the CLOB API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Numeric notification ID. Integer on the wire — upstream's schema agrees
    /// (`id: type: integer`) — so this is not a string despite the `id` name.
    pub id: u64,
    #[serde(rename = "type")]
    pub notification_type: u32,
    /// API key of the notification owner; empty for broadcast notifications.
    pub owner: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    /// RFC3339 timestamp, e.g. `2026-08-03T14:54:12.384486Z`.
    ///
    /// Upstream's schema documents this as `type: integer`, but the venue sends
    /// a string. The wire wins; do not "correct" this to match the spec.
    #[serde(default)]
    pub timestamp: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_deserializes() {
        let json = r#"{
            "id": 1390056400,
            "type": 1,
            "owner": "aa17dfae-754d-2498-f336-8bd1d0e6a1c3",
            "payload": {"order_id": "order-456", "side": "BUY"},
            "timestamp": "2024-01-01T00:00:00Z"
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.id, 1390056400);
        assert_eq!(notif.notification_type, 1);
        assert_eq!(notif.owner, "aa17dfae-754d-2498-f336-8bd1d0e6a1c3");
        assert_eq!(notif.payload["order_id"], "order-456");
        assert_eq!(notif.timestamp.as_deref(), Some("2024-01-01T00:00:00Z"));
    }

    #[test]
    fn notification_null_payload() {
        let json = r#"{
            "id": 1390056401,
            "type": 0,
            "owner": "f4f247b7-4ac7-ff29-a152-04fda0a8755a",
            "payload": null
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.id, 1390056401);
        assert_eq!(notif.notification_type, 0);
        assert!(notif.payload.is_null());
        assert!(notif.timestamp.is_none());
    }

    #[test]
    fn notification_missing_payload() {
        let json = r#"{
            "id": 1390056402,
            "type": 2,
            "owner": ""
        }"#;
        let notif: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.id, 1390056402);
        assert_eq!(notif.notification_type, 2);
        assert!(notif.payload.is_null());
    }

    /// `Notification.id` is an integer on the wire, not a string.
    ///
    /// Captured from a live `GET /notifications` response on 2026-08-03; the
    /// field types below are the observed ones, only the values are sanitised.
    /// Upstream's own schema agrees (`id: type: integer`), so this was wrong
    /// against both the spec and the venue.
    ///
    /// Note `timestamp`: the spec claims `type: integer`, but the wire sends an
    /// RFC3339 **string**. `Option<String>` is therefore correct and is pinned
    /// here so nobody "fixes" it to match the spec.
    ///
    /// The three tests above could not have caught this — they each supply a
    /// string id, so they assert the declaration rather than the venue.
    #[test]
    fn notification_id_is_an_integer_on_the_wire() {
        let json = r#"[{
            "id": 1390056400,
            "type": 2,
            "owner": "aa17dfae-754d-2498-f336-8bd1d0e6a1c3",
            "payload": {"orderId": "0xabc", "outcome": "Yes"},
            "timestamp": "2026-08-03T14:54:12.384486Z"
        }]"#;

        let notifications: Vec<Notification> = match serde_json::from_str(json) {
            Ok(n) => n,
            Err(e) => panic!("a real notification row must deserialize, got: {e}"),
        };

        assert_eq!(notifications.len(), 1);
        let notif = &notifications[0];
        assert_eq!(notif.id, 1_390_056_400, "id must round-trip as an integer");
        assert_eq!(notif.notification_type, 2);
        assert_eq!(notif.owner, "aa17dfae-754d-2498-f336-8bd1d0e6a1c3");
        assert_eq!(notif.payload["outcome"], "Yes");
        assert_eq!(
            notif.timestamp.as_deref(),
            Some("2026-08-03T14:54:12.384486Z"),
            "timestamp is an RFC3339 string on the wire, despite the spec saying integer"
        );
    }
}
