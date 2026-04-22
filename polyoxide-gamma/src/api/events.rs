use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::GammaError,
    types::{CountResponse, Event, Tag},
};

/// Events namespace for event-related operations
#[derive(Clone)]
pub struct Events {
    pub(crate) http_client: HttpClient,
}

impl Events {
    /// List events with optional filtering
    pub fn list(&self) -> ListEvents {
        ListEvents {
            request: Request::new(self.http_client.clone(), "/events"),
        }
    }

    /// Get an event by ID
    pub fn get(&self, id: impl Into<String>) -> GetEvent {
        GetEvent {
            request: Request::new(
                self.http_client.clone(),
                format!("/events/{}", urlencoding::encode(&id.into())),
            ),
        }
    }

    /// Get an event by slug
    pub fn get_by_slug(&self, slug: impl Into<String>) -> GetEvent {
        GetEvent {
            request: Request::new(
                self.http_client.clone(),
                format!("/events/slug/{}", urlencoding::encode(&slug.into())),
            ),
        }
    }

    /// Get tags for an event
    pub fn tags(&self, id: impl Into<String>) -> Request<Vec<Tag>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/events/{}/tags", urlencoding::encode(&id.into())),
        )
    }

    /// Get tweet count for an event
    pub fn tweet_count(&self, id: impl Into<String>) -> Request<CountResponse, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/events/{}/tweet-count", urlencoding::encode(&id.into())),
        )
    }

    /// Get comment count for an event
    pub fn comment_count(&self, id: impl Into<String>) -> Request<CountResponse, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/events/{}/comments/count", urlencoding::encode(&id.into())),
        )
    }
}

/// Request builder for getting a single event
pub struct GetEvent {
    request: Request<Event, GammaError>,
}

impl GetEvent {
    /// Include chat data in response
    pub fn include_chat(mut self, include: bool) -> Self {
        self.request = self.request.query("include_chat", include);
        self
    }

    /// Include template data in response
    pub fn include_template(mut self, include: bool) -> Self {
        self.request = self.request.query("include_template", include);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Event, GammaError> {
        self.request.send().await
    }
}

/// Request builder for listing events
pub struct ListEvents {
    request: Request<Vec<Event>, GammaError>,
}

impl ListEvents {
    /// Set maximum number of results (minimum: 0)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (minimum: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Set order fields (comma-separated list)
    pub fn order(mut self, order: impl Into<String>) -> Self {
        self.request = self.request.query("order", order.into());
        self
    }

    /// Set sort direction
    pub fn ascending(mut self, ascending: bool) -> Self {
        self.request = self.request.query("ascending", ascending);
        self
    }

    /// Filter by specific event IDs
    ///
    /// Safe batch size: ≤ 400 per request. URLs over ~8 KB are rejected
    /// upstream with `414 URI Too Long`.
    pub fn id(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.request = self.request.query_many("id", ids);
        self
    }

    /// Filter by tag identifier
    pub fn tag_id(mut self, tag_id: i64) -> Self {
        self.request = self.request.query("tag_id", tag_id);
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

    /// Filter by event slugs
    ///
    /// Safe batch size: ≤ 100 per request. URL length is capped at ~8 KB
    /// upstream; slug entries vary so pick a cap based on your longest slug.
    pub fn slug(mut self, slugs: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("slug", slugs);
        self
    }

    /// Filter by tag slug
    pub fn tag_slug(mut self, slug: impl Into<String>) -> Self {
        self.request = self.request.query("tag_slug", slug.into());
        self
    }

    /// Include related tags in response
    pub fn related_tags(mut self, include: bool) -> Self {
        self.request = self.request.query("related_tags", include);
        self
    }

    /// Filter active events only
    pub fn active(mut self, active: bool) -> Self {
        self.request = self.request.query("active", active);
        self
    }

    /// Filter archived events
    pub fn archived(mut self, archived: bool) -> Self {
        self.request = self.request.query("archived", archived);
        self
    }

    /// Filter featured events
    pub fn featured(mut self, featured: bool) -> Self {
        self.request = self.request.query("featured", featured);
        self
    }

    /// Filter create-your-own-market events
    pub fn cyom(mut self, cyom: bool) -> Self {
        self.request = self.request.query("cyom", cyom);
        self
    }

    /// Include chat data in response
    pub fn include_chat(mut self, include: bool) -> Self {
        self.request = self.request.query("include_chat", include);
        self
    }

    /// Include template data
    pub fn include_template(mut self, include: bool) -> Self {
        self.request = self.request.query("include_template", include);
        self
    }

    /// Filter by recurrence pattern
    pub fn recurrence(mut self, recurrence: impl Into<String>) -> Self {
        self.request = self.request.query("recurrence", recurrence.into());
        self
    }

    /// Filter closed events
    pub fn closed(mut self, closed: bool) -> Self {
        self.request = self.request.query("closed", closed);
        self
    }

    /// Set minimum liquidity threshold
    pub fn liquidity_min(mut self, min: f64) -> Self {
        self.request = self.request.query("liquidity_min", min);
        self
    }

    /// Set maximum liquidity threshold
    pub fn liquidity_max(mut self, max: f64) -> Self {
        self.request = self.request.query("liquidity_max", max);
        self
    }

    /// Set minimum trading volume
    pub fn volume_min(mut self, min: f64) -> Self {
        self.request = self.request.query("volume_min", min);
        self
    }

    /// Set maximum trading volume
    pub fn volume_max(mut self, max: f64) -> Self {
        self.request = self.request.query("volume_max", max);
        self
    }

    /// Set earliest start date (ISO 8601 format)
    pub fn start_date_min(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("start_date_min", date.into());
        self
    }

    /// Set latest start date (ISO 8601 format)
    pub fn start_date_max(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("start_date_max", date.into());
        self
    }

    /// Set earliest end date (ISO 8601 format)
    pub fn end_date_min(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("end_date_min", date.into());
        self
    }

    /// Set latest end date (ISO 8601 format)
    pub fn end_date_max(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("end_date_max", date.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Event>, GammaError> {
        self.request.send().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    /// Verify that all event builder methods chain correctly
    #[test]
    fn test_list_events_full_chain() {
        let _list = gamma()
            .events()
            .list()
            .limit(10)
            .offset(20)
            .order("volume")
            .ascending(true)
            .id(vec![1i64, 2])
            .tag_id(42)
            .exclude_tag_id(vec![99i64])
            .slug(vec!["slug-a"])
            .tag_slug("politics")
            .related_tags(true)
            .active(true)
            .archived(false)
            .featured(true)
            .cyom(false)
            .include_chat(true)
            .include_template(false)
            .recurrence("daily")
            .closed(false)
            .liquidity_min(1000.0)
            .liquidity_max(50000.0)
            .volume_min(100.0)
            .volume_max(10000.0)
            .start_date_min("2024-01-01")
            .start_date_max("2025-01-01")
            .end_date_min("2024-06-01")
            .end_date_max("2025-12-31");
    }

    #[test]
    fn test_get_event_accepts_str_and_string() {
        let _req1 = gamma().events().get("evt-123");
        let _req2 = gamma().events().get(String::from("evt-123"));
    }

    #[test]
    fn test_get_by_slug_accepts_str_and_string() {
        let _req1 = gamma().events().get_by_slug("slug");
        let _req2 = gamma().events().get_by_slug(String::from("slug"));
    }

    #[test]
    fn test_get_event_with_query_params() {
        let _req = gamma()
            .events()
            .get("evt-123")
            .include_chat(true)
            .include_template(false);
    }

    #[test]
    fn test_event_tags_accepts_str_and_string() {
        let _req1 = gamma().events().tags("evt-123");
        let _req2 = gamma().events().tags(String::from("evt-123"));
    }

    #[test]
    fn test_event_tweet_count() {
        let _req = gamma().events().tweet_count("evt-123");
    }

    #[test]
    fn test_event_comment_count() {
        let _req = gamma().events().comment_count("evt-123");
    }
}
