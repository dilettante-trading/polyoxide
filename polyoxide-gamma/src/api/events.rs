use polyoxide_core::{HttpClient, Request};

use crate::{
    error::GammaError,
    types::{CountResponse, Event, EventCreator, EventsPagination, KeysetEventsResponse, Tag},
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

    /// List event creators with optional filtering
    /// (`GET /events/creators`).
    pub fn list_creators(&self) -> ListEventCreators {
        ListEventCreators {
            request: Request::new(self.http_client.clone(), "/events/creators"),
        }
    }

    /// Get an event creator by ID (`GET /events/creators/{id}`).
    pub fn get_creator(&self, id: impl Into<String>) -> Request<EventCreator, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/events/creators/{}", urlencoding::encode(&id.into())),
        )
    }

    /// List events with offset-style pagination metadata
    /// (`GET /events/pagination`).
    pub fn list_paginated(&self) -> ListPaginatedEvents {
        ListPaginatedEvents {
            request: Request::new(self.http_client.clone(), "/events/pagination"),
        }
    }

    /// List sport event results (`GET /events/results`).
    pub fn list_results(&self) -> ListEventResults {
        ListEventResults {
            request: Request::new(self.http_client.clone(), "/events/results"),
        }
    }

    /// List events using cursor-based (keyset) pagination
    /// (`GET /events/keyset`).
    ///
    /// Prefer this over [`Self::list`] for stable paging through large result
    /// sets. Use `next_cursor` from each response as `after_cursor` in the
    /// next request; pagination is complete when `next_cursor` is `None`.
    ///
    /// Every query parameter upstream documents for this route has a builder
    /// method except `offset`, which the route refuses with `422`.
    pub fn list_keyset(&self) -> ListKeysetEvents {
        ListKeysetEvents {
            request: Request::new(self.http_client.clone(), "/events/keyset"),
        }
    }
}

/// Request builder for [`Events::list_creators`].
pub struct ListEventCreators {
    request: Request<Vec<EventCreator>, GammaError>,
}

impl ListEventCreators {
    polyoxide_core::query_setters! {
        /// Limit the number of results (minimum: 0).
        limit: u32 => "limit",
        /// Pagination offset (minimum: 0).
        offset: u32 => "offset",
        /// Comma-separated list of fields to order by.
        order: impl Into<String> => "order",
        /// Sort direction.
        ascending: bool => "ascending",
        /// Filter by creator name.
        creator_name: impl Into<String> => "creator_name",
        /// Filter by creator handle.
        creator_handle: impl Into<String> => "creator_handle",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<EventCreator>, GammaError> {
        self.request.send().await
    }
}

/// Request builder for [`Events::list_paginated`].
pub struct ListPaginatedEvents {
    request: Request<EventsPagination, GammaError>,
}

impl ListPaginatedEvents {
    polyoxide_core::query_setters! {
        /// Limit the number of results.
        limit: u32 => "limit",
        /// Pagination offset.
        offset: u32 => "offset",
        /// Comma-separated list of fields to order by.
        order: impl Into<String> => "order",
        /// Sort direction.
        ascending: bool => "ascending",
        /// Include chat data in response.
        include_chat: bool => "include_chat",
        /// Include template data in response.
        include_template: bool => "include_template",
        /// Filter by recurrence pattern.
        recurrence: impl Into<String> => "recurrence",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<EventsPagination, GammaError> {
        self.request.send().await
    }
}

/// Request builder for [`Events::list_results`].
pub struct ListEventResults {
    request: Request<Vec<Event>, GammaError>,
}

impl ListEventResults {
    polyoxide_core::query_setters! {
        /// Limit the number of results.
        limit: u32 => "limit",
        /// Pagination offset.
        offset: u32 => "offset",
        /// Comma-separated list of fields to order by.
        order: impl Into<String> => "order",
        /// Sort direction.
        ascending: bool => "ascending",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Event>, GammaError> {
        self.request.send().await
    }
}

/// Request builder for [`Events::list_keyset`].
pub struct ListKeysetEvents {
    request: Request<KeysetEventsResponse, GammaError>,
}

impl ListKeysetEvents {
    polyoxide_core::query_setters! {
        /// Maximum number of results to return (upstream max 100). Larger values
        /// are clamped to 100, not rejected, so a page can be shorter than asked.
        limit: u32 => "limit",
        /// Comma-separated list of JSON field names to order by.
        order: impl Into<String> => "order",
        /// Sort direction (used only when `order` is set).
        ascending: bool => "ascending",
        /// Opaque cursor token returned as `next_cursor` from a previous response.
        after_cursor: impl Into<String> => "after_cursor",
        /// Filter by specific event IDs.
        id: many impl IntoIterator<Item = i64> => "id",
        /// Filter by event slugs.
        slug: many impl IntoIterator<Item = impl ToString> => "slug",
        /// Filter by closed status.
        closed: bool => "closed",
        /// Filter live events only.
        live: bool => "live",
        /// Filter featured events only.
        featured: bool => "featured",
        /// Search by event title substring.
        title_search: impl Into<String> => "title_search",
        /// Filter by tag IDs.
        tag_id: many impl IntoIterator<Item = i64> => "tag_id",
        /// Filter by tag slug.
        tag_slug: impl Into<String> => "tag_slug",
        /// Set minimum liquidity threshold.
        liquidity_min: f64 => "liquidity_min",
        /// Set maximum liquidity threshold.
        liquidity_max: f64 => "liquidity_max",
        /// Set minimum trading volume.
        volume_min: f64 => "volume_min",
        /// Set maximum trading volume.
        volume_max: f64 => "volume_max",
        /// Filter to create-your-own-market events.
        cyom: bool => "cyom",
        /// Filter by minimum start date (RFC3339).
        start_date_min: impl Into<String> => "start_date_min",
        /// Filter by maximum start date (RFC3339).
        start_date_max: impl Into<String> => "start_date_max",
        /// Filter by minimum end date (RFC3339).
        end_date_min: impl Into<String> => "end_date_min",
        /// Filter by maximum end date (RFC3339).
        end_date_max: impl Into<String> => "end_date_max",
        /// Filter by minimum game start time (RFC3339).
        start_time_min: impl Into<String> => "start_time_min",
        /// Filter by maximum game start time (RFC3339).
        start_time_max: impl Into<String> => "start_time_max",
        /// Exclude events carrying any of these tag IDs.
        exclude_tag_id: many impl IntoIterator<Item = i64> => "exclude_tag_id",
        /// Include events matching related tags.
        related_tags: bool => "related_tags",
        /// Tag matching mode.
        tag_match: impl Into<String> => "tag_match",
        /// Filter by series IDs.
        series_id: many impl IntoIterator<Item = i64> => "series_id",
        /// Filter by game IDs.
        game_id: many impl IntoIterator<Item = i64> => "game_id",
        /// Filter by event date (RFC3339).
        event_date: impl Into<String> => "event_date",
        /// Filter by event week number.
        event_week: i64 => "event_week",
        /// Order results by the featured ranking.
        featured_order: bool => "featured_order",
        /// Filter by recurrence.
        recurrence: impl Into<String> => "recurrence",
        /// Filter by creator addresses.
        created_by: many impl IntoIterator<Item = impl ToString> => "created_by",
        /// Filter to children of a specific parent event.
        parent_event_id: i64 => "parent_event_id",
        /// Include child events in the response.
        include_children: bool => "include_children",
        /// Attach `external_partners` for the given partner slug.
        partner_slug: impl Into<String> => "partner_slug",
        /// Include chat data in the response.
        include_chat: bool => "include_chat",
        /// Include template data in the response.
        include_template: bool => "include_template",
        /// Include the `BestLines` relation in the response.
        include_best_lines: bool => "include_best_lines",
        /// Include the nested `markets` array in each event (the server's default).
        /// With `false` the server omits the key, so every `Event::markets` is empty.
        include_markets: bool => "include_markets",
        /// Set the response locale.
        locale: impl Into<String> => "locale",
    }

    // Note: `/events/keyset` documents `offset` as "Not allowed. Returns 422 if
    // provided." — it is deliberately not exposed here. Page with
    // [`after_cursor`](Self::after_cursor) instead.

    /// Execute the request.
    pub async fn send(self) -> Result<KeysetEventsResponse, GammaError> {
        self.request.send().await
    }
}

/// Request builder for getting a single event
pub struct GetEvent {
    request: Request<Event, GammaError>,
}

impl GetEvent {
    polyoxide_core::query_setters! {
        /// Include chat data in response
        include_chat: bool => "include_chat",
        /// Include template data in response
        include_template: bool => "include_template",
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
    polyoxide_core::query_setters! {
        /// Set maximum number of results (minimum: 0)
        limit: u32 => "limit",
        /// Set pagination offset (minimum: 0)
        offset: u32 => "offset",
        /// Set order fields (comma-separated list)
        order: impl Into<String> => "order",
        /// Set sort direction
        ascending: bool => "ascending",
        /// Filter by specific event IDs
        ///
        /// Safe batch size: ≤ 400 per request. URLs over ~8 KB are rejected
        /// upstream with `414 URI Too Long`.
        id: many impl IntoIterator<Item = i64> => "id",
        /// Filter by game IDs, the [`Event::game_id`] a live score from
        /// `polyoxide-sports` carries.
        ///
        /// Not in upstream's `openapi.yaml` for this route, but the server applies
        /// it (verified 2026-10-08): an id with no game returns `[]`, and a
        /// non-integer is refused with `invalid integer`. One id can return
        /// several events, a game and its child events; see
        /// [`Event::parent_event_id`].
        ///
        /// [`Event::game_id`]: crate::types::Event::game_id
        /// [`Event::parent_event_id`]: crate::types::Event::parent_event_id
        game_id: many impl IntoIterator<Item = i64> => "game_id",
        /// Filter by tag identifier
        tag_id: i64 => "tag_id",
        /// Exclude events with specified tag IDs
        ///
        /// Safe batch size: ≤ 500 per request. Tag IDs are short integers
        /// (~5 B/entry); URLs over ~8 KB are rejected upstream with `414`.
        exclude_tag_id: many impl IntoIterator<Item = i64> => "exclude_tag_id",
        /// Filter by event slugs
        ///
        /// Safe batch size: ≤ 100 per request. URL length is capped at ~8 KB
        /// upstream; slug entries vary so pick a cap based on your longest slug.
        slug: many impl IntoIterator<Item = impl ToString> => "slug",
        /// Filter by tag slug
        tag_slug: impl Into<String> => "tag_slug",
        /// Include related tags in response
        related_tags: bool => "related_tags",
        /// Filter active events only
        active: bool => "active",
        /// Filter archived events
        archived: bool => "archived",
        /// Filter featured events
        featured: bool => "featured",
        /// Filter create-your-own-market events
        cyom: bool => "cyom",
        /// Include chat data in response
        include_chat: bool => "include_chat",
        /// Include template data
        include_template: bool => "include_template",
        /// Include the nested `markets` array in each event
        include_markets: bool => "include_markets",
        /// Filter by recurrence pattern
        recurrence: impl Into<String> => "recurrence",
        /// Filter closed events
        closed: bool => "closed",
        /// Set minimum liquidity threshold
        liquidity_min: f64 => "liquidity_min",
        /// Set maximum liquidity threshold
        liquidity_max: f64 => "liquidity_max",
        /// Set minimum trading volume
        volume_min: f64 => "volume_min",
        /// Set maximum trading volume
        volume_max: f64 => "volume_max",
        /// Set earliest start date (ISO 8601 format)
        start_date_min: impl Into<String> => "start_date_min",
        /// Set latest start date (ISO 8601 format)
        start_date_max: impl Into<String> => "start_date_max",
        /// Set earliest end date (ISO 8601 format)
        end_date_min: impl Into<String> => "end_date_min",
        /// Set latest end date (ISO 8601 format)
        end_date_max: impl Into<String> => "end_date_max",
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
            .game_id(vec![10079774i64])
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
            .include_markets(true)
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

    #[test]
    fn test_list_creators_full_chain() {
        let _req = gamma()
            .events()
            .list_creators()
            .limit(10)
            .offset(0)
            .order("createdAt")
            .ascending(true)
            .creator_name("poly")
            .creator_handle("polymarket");
    }

    #[test]
    fn test_get_creator_accepts_str_and_string() {
        let _req1 = gamma().events().get_creator("c-1");
        let _req2 = gamma().events().get_creator(String::from("c-1"));
    }

    #[test]
    fn test_list_paginated_full_chain() {
        let _req = gamma()
            .events()
            .list_paginated()
            .limit(25)
            .offset(50)
            .order("startDate")
            .ascending(false)
            .include_chat(false)
            .include_template(true)
            .recurrence("daily");
    }

    #[test]
    fn test_list_results_full_chain() {
        let _req = gamma()
            .events()
            .list_results()
            .limit(5)
            .offset(0)
            .order("endDate")
            .ascending(true);
    }

    #[test]
    fn test_list_keyset_full_chain() {
        let _req = gamma()
            .events()
            .list_keyset()
            .limit(50)
            .order("volume_num")
            .ascending(true)
            .after_cursor("abc")
            .id(vec![1i64, 2])
            .slug(vec!["slug-a"])
            .closed(false)
            .live(true)
            .featured(true)
            .title_search("bitcoin")
            .tag_id(vec![42i64])
            .tag_slug("politics")
            .liquidity_min(0.0)
            .liquidity_max(1e6)
            .volume_min(0.0)
            .volume_max(1e6);
    }
}
