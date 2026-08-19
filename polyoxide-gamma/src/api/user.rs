use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{error::GammaError, types::Profile};

/// User API namespace
#[derive(Clone)]
pub struct User {
    pub(crate) http_client: HttpClient,
}

impl User {
    /// Get user details (`GET /public-profile`).
    ///
    /// Responses are modelled by [`UserResponse`], which follows the
    /// endpoint's own published `PublicProfileResponse.json` schema — see
    /// that type's docs.
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

/// User details response from `GET /public-profile`.
///
/// Modelled against the endpoint's own published JSON Schema —
/// `https://gamma-api.polymarket.com/schemas/PublicProfileResponse.json`,
/// linked from the response body's `$schema` key — rather than
/// `docs/specs/gamma/openapi.yaml`, which does not describe this endpoint at
/// all. See `docs/specs/gamma/OBSERVED.md`.
///
/// `taker_tier`, `taker_tier_name` and `weighted_volume` are the schema's only
/// `required` properties, and every capture behind
/// `tests/fixtures/user_response_{full,sparse}.json` carried all three. Every
/// other field is optional; the server omits `name`, `pseudonym`,
/// `profile_image`, `bio`, `x_username` and `discord_username` for some
/// subjects — `discord_username` was not observed at all in a 39-address
/// sample (see `tests/fixtures/README.md`). There is no top-level `address`
/// or `id`: the wire never sends either, and the account id lives nested in
/// [`users`](Self::users).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UserResponse {
    /// The user's proxy wallet address (Treasury)
    #[serde(rename = "proxyWallet")]
    pub proxy: Option<String>,
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
    /// Discord handle
    pub discord_username: Option<String>,
    /// Whether the user has a verified badge
    pub verified_badge: Option<bool>,
    /// Taker fee tier
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub taker_tier: i64,
    /// Taker fee tier's display name
    pub taker_tier_name: String,
    /// Volume-weighted trading activity used to compute the taker tier
    pub weighted_volume: f64,
    /// User identity entries. The schema types this `["array","null"]`; an
    /// explicit `null` deserializes the same as an absent key.
    #[serde(default, deserialize_with = "deserialize_users")]
    pub users: Vec<UserInfo>,
}

/// Deserializes [`UserResponse::users`], tolerating the schema's explicit
/// `null` in addition to an absent key (already handled by
/// `#[serde(default)]`).
fn deserialize_users<'de, D>(deserializer: D) -> Result<Vec<UserInfo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let users: Option<Vec<UserInfo>> = Option::deserialize(deserializer)?;
    Ok(users.unwrap_or_default())
}

/// User identity entry returned inside a profile's `users` array
/// (`PublicProfileUser.json`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UserInfo {
    /// Account ID. The schema's only required property on this object.
    pub id: String,
    /// Whether this user is a creator
    #[serde(default)]
    pub creator: bool,
    /// Whether this user is a moderator
    #[serde(rename = "mod", default)]
    pub moderator: bool,
    /// Whether this user is a community moderator. Absent from the wire for
    /// some subjects (1 of 39 sampled nested `users[]` entries), so this
    /// defaults to `false` rather than requiring the key.
    #[serde(default)]
    pub community_mod: bool,
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
