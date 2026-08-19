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
        let request =
            Request::new(self.http_client.clone(), "/public-search").query("q", query.into());
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

    /// Filter by event status
    pub fn events_status(mut self, status: impl Into<String>) -> Self {
        self.request = self.request.query("events_status", status.into());
        self
    }

    /// Filter by event tag IDs
    ///
    /// Safe batch size: ≤ 200 per request. URLs over ~8 KB are rejected
    /// upstream with `414 URI Too Long`.
    pub fn events_tag(mut self, tag_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("events_tag", tag_ids);
        self
    }

    /// Include closed markets in results
    pub fn keep_closed_markets(mut self, keep: i32) -> Self {
        self.request = self.request.query("keep_closed_markets", keep);
        self
    }

    /// Set sort order
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.request = self.request.query("sort", sort.into());
        self
    }

    /// Sort direction (used only when [`sort`](Self::sort) is set).
    pub fn ascending(mut self, ascending: bool) -> Self {
        self.request = self.request.query("ascending", ascending);
        self
    }

    /// Include tag search results
    pub fn search_tags(mut self, include: bool) -> Self {
        self.request = self.request.query("search_tags", include);
        self
    }

    /// Filter by recurrence pattern
    pub fn recurrence(mut self, recurrence: impl Into<String>) -> Self {
        self.request = self.request.query("recurrence", recurrence.into());
        self
    }

    /// Exclude events with specified tag IDs
    ///
    /// Safe batch size: ≤ 500 per request. Tag IDs are short integers
    /// (~5 B/entry); URLs over ~8 KB are rejected upstream with `414`.
    pub fn exclude_tag_id(mut self, tag_ids: impl IntoIterator<Item = i64>) -> Self {
        self.request = self.request.query_many("exclude_tag_id", tag_ids);
        self
    }

    /// Enable optimized search
    pub fn optimized(mut self, optimized: bool) -> Self {
        self.request = self.request.query("optimized", optimized);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<SearchResponse, GammaError> {
        self.request.send().await
    }
}

/// Response from public search (`GET /public-search`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// Matching user profiles.
    ///
    /// The server can send a JSON `null` for individual entries in this
    /// array in place of a profile object, in addition to omitting the whole
    /// key (already handled by `#[serde(default)]`) — reproducible with
    /// `q=sports&search_profiles=true&limit_per_type=20`, stable across 5/5
    /// attempts on 2026-08-19 (see `docs/specs/gamma/OBSERVED.md` and
    /// `tests/fixtures/README.md`). Each element is therefore
    /// `Option<SearchProfile>`, not `SearchProfile` — `Option<T>`'s own
    /// `Deserialize` impl decodes a `null` element as `None`, so no custom
    /// deserializer is needed, but a caller that assumes every entry is
    /// populated must filter or unwrap.
    #[serde(default)]
    pub profiles: Vec<Option<SearchProfile>>,
    /// Matching events
    #[serde(default)]
    pub events: Vec<Event>,
    /// Matching tags
    #[serde(default)]
    pub tags: Vec<Tag>,
}

/// Profile result from search (`GET /public-search`).
///
/// Unlike `/public-profile` and `/profiles/user_address/{address}`,
/// `/public-search` serves no `$schema` key — there is no published contract
/// to model this type against. Modelled instead from a 228-profile,
/// 12-query live sample (2026-08-19; see `tests/fixtures/README.md`):
/// `name`, `display_username_public` and `proxy_wallet` were present on
/// every sampled profile and `pseudonym` on all but 5, but that is a strong
/// sample, not a guarantee the way a schema's `required` list would be, so
/// every field here stays `Option`. There is no `address` field: the old one
/// was invented — 0 of 228 sampled profiles carried it — and the value
/// callers actually want is [`proxy_wallet`](Self::proxy_wallet).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SearchProfile {
    /// Display name
    pub name: Option<String>,
    /// Whether the username is displayed publicly
    pub display_username_public: Option<bool>,
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
            .cache(false)
            .events_status("active")
            .events_tag(vec![1i64, 2])
            .keep_closed_markets(0)
            .sort("volume")
            .search_tags(true)
            .recurrence("daily")
            .exclude_tag_id(vec![99i64])
            .optimized(true);
    }

    #[test]
    fn test_search_response_deserialization() {
        // Shape captured live from `/public-search` — no `address` key; the
        // server has never sent one (see tests/wire_agreement.rs).
        let json = r#"{
            "profiles": [
                {
                    "name": "trader1",
                    "displayUsernamePublic": true,
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
        let profile = resp.profiles[0].as_ref().expect("entry is not null");
        assert_eq!(profile.name.as_deref(), Some("trader1"));
        assert_eq!(profile.display_username_public, Some(true));
        assert!(resp.events.is_empty());
        assert!(resp.tags.is_empty());
    }

    #[test]
    fn test_search_response_tolerates_null_profile_entry() {
        // `/public-search` sends a JSON `null` for some entries in `profiles`
        // — see tests/wire_agreement.rs's
        // `search_response_tolerates_null_profile_entries`, which pins this
        // against a captured payload. This is the hand-written analogue.
        let json = r#"{
            "profiles": [
                {"name": "trader1", "proxyWallet": "0xproxy"},
                null
            ],
            "events": [],
            "tags": []
        }"#;
        let resp: SearchResponse =
            serde_json::from_str(json).expect("a null profile entry must deserialize, not error");
        assert_eq!(resp.profiles.len(), 2);
        assert!(resp.profiles[0].is_some());
        assert!(resp.profiles[1].is_none());
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
            "name": "Searcher",
            "displayUsernamePublic": true,
            "profileImage": "https://img.example.com/pic.png",
            "pseudonym": "anon",
            "bio": "A bio",
            "proxyWallet": "0xproxy123"
        }"#;
        let profile: SearchProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name.as_deref(), Some("Searcher"));
        assert_eq!(profile.display_username_public, Some(true));
        assert_eq!(profile.bio.as_deref(), Some("A bio"));
        assert_eq!(profile.proxy_wallet.as_deref(), Some("0xproxy123"));
    }

    #[test]
    fn test_search_profile_all_null() {
        let json = r#"{}"#;
        let profile: SearchProfile = serde_json::from_str(json).unwrap();
        assert!(profile.name.is_none());
        assert!(profile.display_username_public.is_none());
        assert!(profile.profile_image.is_none());
        assert!(profile.pseudonym.is_none());
        assert!(profile.bio.is_none());
        assert!(profile.proxy_wallet.is_none());
    }
}
