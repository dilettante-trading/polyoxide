use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{
    error::GammaError,
    types::{Event, Tag},
};

/// Search namespace for search operations
#[derive(Clone)]
pub struct Search {
    pub(crate) http_client: HttpClient,
}

impl Search {
    /// Search profiles, events, and tags
    pub fn public_search(&self, query: impl Into<String>) -> PublicSearch {
        let request = Request::new(self.http_client.clone(), "/public-search")
            .query("q", query.into());
        PublicSearch { request }
    }
}

/// Request builder for public search
pub struct PublicSearch {
    request: Request<SearchResponse, GammaError>,
}

impl PublicSearch {
    /// Include profile results in search
    pub fn search_profiles(mut self, include: bool) -> Self {
        self.request = self.request.query("search_profiles", include);
        self
    }

    /// Set maximum results per type
    pub fn limit_per_type(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit_per_type", limit);
        self
    }

    /// Set page number
    pub fn page(mut self, page: u32) -> Self {
        self.request = self.request.query("page", page);
        self
    }

    /// Enable/disable caching
    pub fn cache(mut self, cache: bool) -> Self {
        self.request = self.request.query("cache", cache);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<SearchResponse, GammaError> {
        self.request.send().await
    }
}

/// Response from public search
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// Matching user profiles
    #[serde(default)]
    pub profiles: Vec<SearchProfile>,
    /// Matching events
    #[serde(default)]
    pub events: Vec<Event>,
    /// Matching tags
    #[serde(default)]
    pub tags: Vec<Tag>,
}

/// Profile result from search
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProfile {
    /// User address
    pub address: Option<String>,
    /// Display name
    pub name: Option<String>,
    /// Profile image URL
    pub profile_image: Option<String>,
    /// User pseudonym
    pub pseudonym: Option<String>,
    /// User biography
    pub bio: Option<String>,
    /// Proxy wallet address
    pub proxy_wallet: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    #[test]
    fn test_public_search_full_chain() {
        let _search = gamma()
            .search()
            .public_search("bitcoin")
            .search_profiles(true)
            .limit_per_type(10)
            .page(1)
            .cache(false);
    }

    #[test]
    fn test_search_response_deserialization() {
        let json = r#"{
            "profiles": [
                {
                    "address": "0xabc",
                    "name": "trader1",
                    "profileImage": null,
                    "pseudonym": null,
                    "bio": null,
                    "proxyWallet": "0xproxy"
                }
            ],
            "events": [],
            "tags": []
        }"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.profiles.len(), 1);
        assert_eq!(resp.profiles[0].address.as_deref(), Some("0xabc"));
        assert!(resp.events.is_empty());
        assert!(resp.tags.is_empty());
    }

    #[test]
    fn test_search_response_empty() {
        let json = r#"{"profiles": [], "events": [], "tags": []}"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(resp.profiles.is_empty());
    }

    #[test]
    fn test_search_response_missing_fields() {
        let json = r#"{}"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(resp.profiles.is_empty());
        assert!(resp.events.is_empty());
        assert!(resp.tags.is_empty());
    }

    #[test]
    fn test_search_profile_deserialization() {
        let json = r#"{
            "address": "0x123",
            "name": "Searcher",
            "profileImage": "https://img.example.com/pic.png",
            "pseudonym": "anon",
            "bio": "A bio",
            "proxyWallet": "0xproxy123"
        }"#;
        let profile: SearchProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.address.as_deref(), Some("0x123"));
        assert_eq!(profile.name.as_deref(), Some("Searcher"));
        assert_eq!(profile.bio.as_deref(), Some("A bio"));
        assert_eq!(profile.proxy_wallet.as_deref(), Some("0xproxy123"));
    }

    #[test]
    fn test_search_profile_all_null() {
        let json = r#"{}"#;
        let profile: SearchProfile = serde_json::from_str(json).unwrap();
        assert!(profile.address.is_none());
        assert!(profile.name.is_none());
        assert!(profile.profile_image.is_none());
        assert!(profile.pseudonym.is_none());
        assert!(profile.bio.is_none());
        assert!(profile.proxy_wallet.is_none());
    }
}
