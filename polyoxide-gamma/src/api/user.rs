use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{error::GammaError, types::Profile};

/// User API namespace
#[derive(Clone)]
pub struct User {
    pub(crate) http_client: HttpClient,
}

impl User {
    /// Get user details
    pub fn get(&self, signer_address: impl Into<String>) -> Request<UserResponse, GammaError> {
        Request::new(self.http_client.clone(), "/public-profile")
            .query("address", signer_address.into())
    }

    /// Get a public profile by wallet address
    /// (`GET /profiles/user_address/{user_address}`).
    ///
    /// `address` must be a 0x-prefixed EVM address. Responses are modelled by
    /// [`Profile`], which follows the endpoint's own published
    /// `PublicProfile.json` schema — not `docs/specs/gamma/openapi.yaml`'s
    /// `Profile`, which describes an unrelated object.
    pub fn get_by_address(&self, address: impl Into<String>) -> Request<Profile, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!(
                "/profiles/user_address/{}",
                urlencoding::encode(&address.into())
            ),
        )
    }
}

/// User details response from the `/public-profile` endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    /// The user's proxy wallet address (Treasury)
    #[serde(rename = "proxyWallet")]
    pub proxy: Option<String>,
    /// The user's EOA address (Signer)
    pub address: Option<String>,
    /// Account ID
    pub id: Option<String>,
    /// Username/Display name
    pub name: Option<String>,
    /// Account creation timestamp (ISO 8601)
    pub created_at: Option<String>,
    /// Profile image URL
    pub profile_image: Option<String>,
    /// Whether the username is displayed publicly
    pub display_username_public: Option<bool>,
    /// User biography
    pub bio: Option<String>,
    /// Auto-generated pseudonym
    pub pseudonym: Option<String>,
    /// Twitter/X handle
    pub x_username: Option<String>,
    /// Whether the user has a verified badge
    pub verified_badge: Option<bool>,
    /// User identity entries
    #[serde(default)]
    pub users: Vec<UserInfo>,
}

/// User identity entry returned inside a profile's `users` array
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    /// User ID
    pub id: Option<String>,
    /// Whether this user is a creator
    #[serde(default)]
    pub creator: bool,
    /// Whether this user is a moderator
    #[serde(rename = "mod")]
    #[serde(default)]
    pub moderator: bool,
}

#[cfg(test)]
mod tests {
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    #[test]
    fn test_get_by_address_accepts_str_and_string() {
        let _r1 = gamma().user().get_by_address("0xdeadbeef");
        let _r2 = gamma().user().get_by_address(String::from("0xdeadbeef"));
    }
}
