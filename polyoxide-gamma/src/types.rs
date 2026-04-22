use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Market data from Gamma API
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    pub id: String,
    pub condition_id: String,
    #[serde(rename = "questionID")]
    pub question_id: Option<String>,
    pub slug: Option<String>,
    #[serde(default)]
    pub tokens: Vec<MarketToken>,
    #[cfg_attr(feature = "specta", specta(type = Option<HashMap<String, String>>))]
    pub rewards: Option<HashMap<String, serde_json::Value>>,
    pub minimum_order_size: Option<String>,
    pub minimum_tick_size: Option<String>,
    pub description: String,
    pub category: Option<String>,
    pub end_date_iso: Option<String>,
    pub start_date_iso: Option<String>,
    pub question: String,
    pub min_incentive_size: Option<String>,
    pub max_incentive_spread: Option<String>,
    #[serde(rename = "submitted_by")]
    pub submitted_by: Option<String>,
    #[serde(rename = "volume24hr")] // lowercase 'hr' to match API
    pub volume_24hr: Option<f64>,
    #[serde(rename = "volume1wk")] // lowercase 'wk' to match API
    pub volume_1wk: Option<f64>,
    #[serde(rename = "volume1mo")] // lowercase 'mo' to match API
    pub volume_1mo: Option<f64>,
    #[serde(rename = "volume1yr")] // lowercase 'yr' to match API
    pub volume_1yr: Option<f64>,
    pub liquidity: Option<String>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    pub neg_risk: Option<bool>,
    pub neg_risk_market_id: Option<String>,
    pub neg_risk_request_id: Option<String>,
    // Use i64 instead of u64 to prevent sentinel value
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub comment_count: Option<i64>,
    pub twitter_card_image: Option<String>,
    pub resolution_source: Option<String>,
    pub amm_type: Option<String>,
    pub sponsor_name: Option<String>,
    pub sponsor_image: Option<String>,
    pub x_axis_value: Option<String>,
    pub y_axis_value: Option<String>,
    #[serde(rename = "denomationToken")]
    pub denomination_token: Option<String>,
    pub fee: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub lower_bound: Option<String>,
    pub upper_bound: Option<String>,
    pub outcomes: Option<String>,
    pub outcome_prices: Option<String>,
    pub volume: Option<String>,
    pub active: Option<bool>,
    pub market_type: Option<String>,
    pub format_type: Option<String>,
    pub lower_bound_date: Option<String>,
    pub upper_bound_date: Option<String>,
    pub closed: Option<bool>,
    pub market_maker_address: String,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub created_by: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub updated_by: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_time: Option<String>,
    pub wide_format: Option<bool>,
    pub new: Option<bool>,
    pub mailchimp_tag: Option<String>,
    pub featured: Option<bool>,
    pub archived: Option<bool>,
    pub resolved_by: Option<String>,
    pub restricted: Option<bool>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub market_group: Option<i64>,
    pub group_item_title: Option<String>,
    pub group_item_threshold: Option<String>,
    pub uma_end_date: Option<String>,
    pub uma_resolution_status: Option<String>,
    pub uma_end_date_iso: Option<String>,
    pub uma_resolution_statuses: Option<String>,
    pub enable_order_book: Option<bool>,
    pub order_price_min_tick_size: Option<f64>,
    pub order_min_size: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub curation_order: Option<i64>,
    pub volume_num: Option<f64>,
    pub liquidity_num: Option<f64>,
    pub has_reviewed_dates: Option<bool>,
    pub ready_for_cron: Option<bool>,
    pub comments_enabled: Option<bool>,
    pub game_start_time: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub seconds_delay: Option<i64>,
    pub clob_token_ids: Option<String>,
    pub disqus_thread: Option<String>,
    pub short_outcomes: Option<String>,
    pub team_aid: Option<String>,
    pub team_bid: Option<String>,
    pub uma_bond: Option<String>,
    pub uma_reward: Option<String>,
    pub fpmm_live: Option<bool>,
    #[serde(rename = "volume24hrAmm")] // Match API field names
    pub volume_24hr_amm: Option<f64>,
    #[serde(rename = "volume1wkAmm")]
    pub volume_1wk_amm: Option<f64>,
    #[serde(rename = "volume1moAmm")]
    pub volume_1mo_amm: Option<f64>,
    #[serde(rename = "volume1yrAmm")]
    pub volume_1yr_amm: Option<f64>,
    #[serde(rename = "volume24hrClob")]
    pub volume_24hr_clob: Option<f64>,
    #[serde(rename = "volume1wkClob")]
    pub volume_1wk_clob: Option<f64>,
    #[serde(rename = "volume1moClob")]
    pub volume_1mo_clob: Option<f64>,
    #[serde(rename = "volume1yrClob")]
    pub volume_1yr_clob: Option<f64>,
    pub volume_amm: Option<f64>,
    pub volume_clob: Option<f64>,
    pub liquidity_amm: Option<f64>,
    pub liquidity_clob: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub maker_base_fee: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub taker_base_fee: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub custom_liveness: Option<i64>,
    pub accepting_orders: Option<bool>,
    pub notifications_enabled: Option<bool>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub score: Option<i64>,
    pub creator: Option<String>,
    pub ready: Option<bool>,
    pub funded: Option<bool>,
    pub past_slugs: Option<String>,
    pub ready_timestamp: Option<String>,
    pub funded_timestamp: Option<String>,
    pub accepting_orders_timestamp: Option<String>,
    pub competitive: Option<f64>,
    pub rewards_min_size: Option<f64>,
    pub rewards_max_spread: Option<f64>,
    pub spread: Option<f64>,
    pub automatically_resolved: Option<bool>,
    pub automatically_active: Option<bool>,
    pub one_day_price_change: Option<f64>,
    pub one_hour_price_change: Option<f64>,
    pub one_week_price_change: Option<f64>,
    pub one_month_price_change: Option<f64>,
    pub one_year_price_change: Option<f64>,
    pub last_trade_price: Option<f64>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub clear_book_on_start: Option<bool>,
    pub chart_color: Option<String>,
    pub series_color: Option<String>,
    pub show_gmp_series: Option<bool>,
    pub show_gmp_outcome: Option<bool>,
    pub manual_activation: Option<bool>,
    pub neg_risk_other: Option<bool>,
    pub game_id: Option<String>,
    pub group_item_range: Option<String>,
    pub sports_market_type: Option<String>,
    pub line: Option<f64>,
    pub pending_deployment: Option<bool>,
    pub deploying: Option<bool>,
    pub deploying_timestamp: Option<String>,
    pub schedule_deployment_timestamp: Option<String>,
    pub rfq_enabled: Option<bool>,
    pub event_start_time: Option<String>,
}

/// Market token (outcome)
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketToken {
    pub token_id: String,
    pub outcome: String,
    pub price: Option<String>,
    pub winner: Option<bool>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub ticker: Option<String>,
    pub slug: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub resolution_source: Option<String>,
    pub start_date: Option<String>,
    pub creation_date: Option<String>,
    pub end_date: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub start_date_iso: Option<String>,
    pub end_date_iso: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub new: Option<bool>,
    pub featured: Option<bool>,
    pub restricted: Option<bool>,
    pub liquidity: Option<f64>,
    pub open_interest: Option<f64>,
    pub sort_by: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub is_template: Option<bool>,
    pub template_variables: Option<String>,
    #[serde(rename = "published_at")]
    pub published_at: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub comments_enabled: Option<bool>,
    pub competitive: Option<f64>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    #[serde(rename = "volume1wk")]
    pub volume_1wk: Option<f64>,
    #[serde(rename = "volume1mo")]
    pub volume_1mo: Option<f64>,
    #[serde(rename = "volume1yr")]
    pub volume_1yr: Option<f64>,
    pub featured_image: Option<String>,
    pub disqus_thread: Option<String>,
    pub parent_event: Option<String>,
    pub enable_order_book: Option<bool>,
    pub liquidity_amm: Option<f64>,
    pub liquidity_clob: Option<f64>,
    pub neg_risk: Option<bool>,
    pub neg_risk_market_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub neg_risk_fee_bips: Option<i64>,
    #[serde(default)]
    pub sub_events: Vec<String>,
    #[serde(default)]
    pub markets: Vec<Market>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub series: Vec<SeriesInfo>,
    pub cyom: Option<bool>,
    pub closed_time: Option<String>,
    pub show_all_outcomes: Option<bool>,
    pub show_market_images: Option<bool>,
    pub automatically_resolved: Option<bool>,
    #[serde(rename = "enableNegRisk")]
    pub enable_neg_risk: Option<bool>,
    pub automatically_active: Option<bool>,
    pub event_date: Option<String>,
    pub start_time: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub event_week: Option<i64>,
    pub series_slug: Option<String>,
    pub score: Option<String>,
    pub elapsed: Option<String>,
    pub period: Option<String>,
    pub live: Option<bool>,
    pub ended: Option<bool>,
    pub finished_timestamp: Option<String>,
    pub gmp_chart_mode: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub tweet_count: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub featured_order: Option<i64>,
    pub estimate_value: Option<bool>,
    pub cant_estimate: Option<bool>,
    pub spreads_main_line: Option<f64>,
    pub totals_main_line: Option<f64>,
    pub carousel_map: Option<String>,
    pub pending_deployment: Option<bool>,
    pub deploying: Option<bool>,
    pub deploying_timestamp: Option<String>,
    pub schedule_deployment_timestamp: Option<String>,
    pub game_status: Option<String>,
}

/// Series information within an event
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesInfo {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub ticker: Option<String>,
    pub series_type: Option<String>,
    pub recurrence: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub layout: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub new: Option<bool>,
    pub featured: Option<bool>,
    pub restricted: Option<bool>,
    pub published_at: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub comments_enabled: Option<bool>,
    pub competitive: Option<String>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    pub start_date: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub comment_count: Option<i64>,
    pub requires_translation: Option<bool>,
}

/// Series data (tournament/season grouping)
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesData {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    pub volume: Option<f64>,
    pub liquidity: Option<f64>,
    #[serde(default)]
    pub events: Vec<Event>,
    pub competitive: Option<String>,
}

/// Abridged series payload returned from `/series-summary/*`.
///
/// Note the mixed casing: `eventDates` / `eventWeeks` are camelCase while
/// `earliest_open_week` / `earliest_open_date` are snake_case in the upstream
/// response, so this struct does not use `#[serde(rename_all = "camelCase")]`
/// and instead spells each field out explicitly.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeriesSummary {
    pub id: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "eventDates", default)]
    pub event_dates: Vec<String>,
    #[cfg_attr(feature = "specta", specta(type = Vec<f64>))]
    #[serde(rename = "eventWeeks", default)]
    pub event_weeks: Vec<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub earliest_open_week: Option<i64>,
    pub earliest_open_date: Option<String>,
}

/// Profile returned from `/profiles/user_address/{user_address}`.
///
/// All fields except `id` are optional; upstream frequently omits UTM
/// attribution and certification fields.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub user: Option<i64>,
    pub referral: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub created_by: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub updated_by: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub utm_term: Option<String>,
    pub wallet_activated: Option<bool>,
    pub pseudonym: Option<String>,
    pub display_username_public: Option<bool>,
    pub profile_image: Option<String>,
    pub bio: Option<String>,
    pub proxy_wallet: Option<String>,
    /// ImageOptimization payload; kept as raw JSON since the upstream shape
    /// is not yet modelled in this crate.
    #[cfg_attr(feature = "specta", specta(skip))]
    pub profile_image_optimized: Option<serde_json::Value>,
    pub is_close_only: Option<bool>,
    pub is_cert_req: Option<bool>,
    pub cert_req_date: Option<String>,
}

/// Tag for categorizing markets/events
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub slug: String,
    pub label: String,
    pub force_show: Option<bool>,
    pub published_at: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub created_by: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub updated_by: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub force_hide: Option<bool>,
    pub is_carousel: Option<bool>,
}

/// Sports metadata
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SportMetadata {
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub id: u64,
    pub sport: String,
    pub image: Option<String>,
    pub resolution: Option<String>,
    pub ordering: Option<String>,
    pub tags: Option<String>,
    pub series: Option<String>,
    pub created_at: Option<String>,
}

/// Sports team
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub id: i64,
    pub name: Option<String>,
    pub league: Option<String>,
    pub record: Option<String>,
    pub logo: Option<String>,
    pub abbreviation: Option<String>,
    pub alias: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Comment on a market/event/series
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub user: CommentUser,
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub series_id: Option<String>,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub reactions: Vec<CommentReaction>,
    #[serde(default)]
    pub positions: Vec<CommentPosition>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub like_count: u32,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub dislike_count: u32,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub reply_count: u32,
}

/// User who created a comment
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentUser {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
}

/// Reaction to a comment
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentReaction {
    pub user_id: String,
    pub reaction_type: String,
}

/// Position held by comment author
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentPosition {
    pub token_id: String,
    pub outcome: String,
    pub shares: String,
}

/// Generic count response (used for tweet count, comment count, etc.)
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountResponse {
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub count: u64,
}

/// Pagination cursor for list operations
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursor {
    pub next_cursor: Option<String>,
}

/// Paginated response wrapper
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
}

/// Event creator metadata returned from `/events/creators*` endpoints.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventCreator {
    pub id: String,
    pub creator_name: Option<String>,
    pub creator_handle: Option<String>,
    pub creator_url: Option<String>,
    pub creator_image: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Offset-style pagination metadata accompanying `EventsPagination`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub has_more: Option<bool>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub total_results: Option<i64>,
}

/// Paginated response from `GET /events/pagination`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsPagination {
    #[serde(default)]
    pub data: Vec<Event>,
    pub pagination: Option<Pagination>,
}

/// Keyset-paginated events response from `GET /events/keyset`.
///
/// `next_cursor` is `None` on the last page. The upstream JSON key is
/// `next_cursor` (snake_case), not `nextCursor`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysetEventsResponse {
    #[serde(default)]
    pub events: Vec<Event>,
    pub next_cursor: Option<String>,
}

/// Response body from `GET /markets/{id}/description`.
///
/// The endpoint returns `{ "description": "..." }`. The field is modelled as
/// `Option<String>` so markets without a description still deserialize.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketDescription {
    pub description: Option<String>,
}

/// JSON request body for `POST /markets/information` and `POST /markets/abridged`.
///
/// Fields are all optional; only set the filters you need. `Vec` fields are
/// omitted from the serialized payload when empty, and `Option` fields when
/// `None`, so a default-constructed body serializes to `{}`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketsInformationBody {
    /// Filter by market numeric IDs.
    #[cfg_attr(feature = "specta", specta(type = Vec<f64>))]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub id: Vec<i64>,
    /// Filter by market slugs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slug: Vec<String>,
    /// When set, restrict to markets with this closed flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    /// Filter by CLOB token IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clob_token_ids: Vec<String>,
    /// Filter by condition IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_ids: Vec<String>,
    /// Filter by market-maker contract addresses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub market_maker_address: Vec<String>,
    /// Minimum liquidity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_num_min: Option<f64>,
    /// Maximum liquidity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_num_max: Option<f64>,
    /// Minimum volume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_num_min: Option<f64>,
    /// Maximum volume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_num_max: Option<f64>,
    /// Minimum start date (ISO-8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_min: Option<String>,
    /// Maximum start date (ISO-8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_max: Option<String>,
    /// Minimum end date (ISO-8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_min: Option<String>,
    /// Maximum end date (ISO-8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date_max: Option<String>,
    /// Include related-tag matches when filtering by `tag_id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_tags: Option<bool>,
    /// Tag numeric id to filter by.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_id: Option<i64>,
    /// Filter to "create your own market" markets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyom: Option<bool>,
    /// UMA resolution status filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uma_resolution_status: Option<String>,
    /// Game ID filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    /// Restrict to these sports market types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sports_market_types: Vec<String>,
    /// Minimum reward size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewards_min_size: Option<f64>,
    /// Filter by question IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub question_ids: Vec<String>,
    /// Include tags in response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_tags: Option<bool>,
}

/// Keyset-paginated markets response from `GET /markets/keyset`.
///
/// `next_cursor` is `None` on the last page. Like `KeysetEventsResponse`, the
/// upstream JSON key is `next_cursor` (snake_case), not `nextCursor`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysetMarketsResponse {
    #[serde(default)]
    pub markets: Vec<Market>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MarketToken ─────────────────────────────────────────────

    #[test]
    fn test_market_token_deserialization() {
        let json = r#"{
            "tokenId": "71321045679252212594626385532706912750332728571942532289631379312455583992563",
            "outcome": "Yes",
            "price": "0.55",
            "winner": false
        }"#;
        let token: MarketToken = serde_json::from_str(json).unwrap();
        assert_eq!(token.outcome, "Yes");
        assert_eq!(token.price.as_deref(), Some("0.55"));
        assert_eq!(token.winner, Some(false));
    }

    #[test]
    fn test_market_token_optional_fields() {
        let json = r#"{"tokenId": "123", "outcome": "No"}"#;
        let token: MarketToken = serde_json::from_str(json).unwrap();
        assert!(token.price.is_none());
        assert!(token.winner.is_none());
    }

    // ── Tag ─────────────────────────────────────────────────────

    #[test]
    fn test_tag_deserialization() {
        let json = r#"{
            "id": "42",
            "slug": "politics",
            "label": "Politics",
            "forceShow": true,
            "publishedAt": "2024-01-01T00:00:00Z",
            "createdBy": 1,
            "updatedBy": 2,
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-06-01T00:00:00Z",
            "forceHide": false,
            "isCarousel": true
        }"#;
        let tag: Tag = serde_json::from_str(json).unwrap();
        assert_eq!(tag.slug, "politics");
        assert_eq!(tag.force_show, Some(true));
        assert_eq!(tag.is_carousel, Some(true));
    }

    #[test]
    fn test_tag_minimal() {
        let json = r#"{"id": "1", "slug": "test", "label": "Test"}"#;
        let tag: Tag = serde_json::from_str(json).unwrap();
        assert_eq!(tag.label, "Test");
        assert!(tag.force_show.is_none());
        assert!(tag.created_by.is_none());
    }

    // ── Market ──────────────────────────────────────────────────

    #[test]
    fn test_market_minimal_deserialization() {
        let json = r#"{
            "id": "12345",
            "conditionId": "0xabc",
            "description": "Will X happen?",
            "question": "Will X happen by end of 2025?",
            "marketMakerAddress": "0x1234567890abcdef"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.id, "12345");
        assert_eq!(market.condition_id, "0xabc");
        assert!(market.tokens.is_empty()); // #[serde(default)]
        assert!(market.tags.is_empty()); // #[serde(default)]
        assert!(market.slug.is_none());
        assert!(market.volume_24hr.is_none());
    }

    #[test]
    fn test_market_with_tokens() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "tokens": [
                {"tokenId": "t1", "outcome": "Yes", "price": "0.7", "winner": true},
                {"tokenId": "t2", "outcome": "No", "price": "0.3", "winner": false}
            ]
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.tokens.len(), 2);
        assert_eq!(market.tokens[0].outcome, "Yes");
        assert_eq!(market.tokens[1].price.as_deref(), Some("0.3"));
    }

    #[test]
    fn test_market_volume_fields() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "volume24hr": 1500.5,
            "volume1wk": 10000.0,
            "volume1mo": 50000.0,
            "volume1yr": 200000.0,
            "volume24hrAmm": 100.0,
            "volume1wkClob": 9900.0
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.volume_24hr, Some(1500.5));
        assert_eq!(market.volume_1wk, Some(10000.0));
        assert_eq!(market.volume_24hr_amm, Some(100.0));
        assert_eq!(market.volume_1wk_clob, Some(9900.0));
    }

    #[test]
    fn test_market_denomination_token_rename() {
        // API field is "denomationToken" (typo in Polymarket API)
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "denomationToken": "USDC"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.denomination_token.as_deref(), Some("USDC"));
    }

    #[test]
    fn test_market_rewards_as_map() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "rewards": {"min_size": "100", "max_spread": "0.05"}
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert!(market.rewards.is_some());
        let rewards = market.rewards.unwrap();
        assert_eq!(rewards["min_size"], "100");
    }

    #[test]
    fn test_market_null_rewards() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "rewards": null
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert!(market.rewards.is_none());
    }

    // ── Event ───────────────────────────────────────────────────

    #[test]
    fn test_event_minimal() {
        let json = r#"{"id": "evt-1"}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.id, "evt-1");
        assert!(event.markets.is_empty()); // #[serde(default)]
        assert!(event.tags.is_empty());
        assert!(event.series.is_empty());
        assert!(event.sub_events.is_empty());
    }

    #[test]
    fn test_event_with_nested_markets() {
        let json = r#"{
            "id": "evt-1",
            "title": "2025 Election",
            "markets": [
                {
                    "id": "mkt-1",
                    "conditionId": "0xabc",
                    "description": "Who wins?",
                    "question": "Who wins the election?",
                    "marketMakerAddress": "0xaddr"
                }
            ]
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.markets.len(), 1);
        assert_eq!(event.markets[0].id, "mkt-1");
    }

    #[test]
    fn test_event_volume_24h_rename() {
        let json = r#"{
            "id": "evt-1",
            "volume24hr": 5000.0
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.volume_24hr, Some(5000.0));
    }

    #[test]
    fn test_event_volume_24h_old_key_ignored() {
        // The old key "volume24h" should NOT deserialize into volume_24hr
        let json = r#"{
            "id": "evt-1",
            "volume24h": 5000.0
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.volume_24hr, None);
    }

    #[test]
    fn test_event_enable_neg_risk_rename() {
        let json = r#"{
            "id": "evt-1",
            "enableNegRisk": true
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.enable_neg_risk, Some(true));
    }

    #[test]
    fn test_event_published_at_snake_case() {
        let json = r#"{
            "id": "evt-1",
            "published_at": "2024-01-01T00:00:00Z"
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.published_at.as_deref(), Some("2024-01-01T00:00:00Z"));
    }

    #[test]
    fn test_market_question_id_capital() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "questionID": "0xabc123"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.question_id.as_deref(), Some("0xabc123"));
    }

    #[test]
    fn test_market_has_reviewed_dates() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "hasReviewedDates": true
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.has_reviewed_dates, Some(true));
    }

    #[test]
    fn test_market_rewards_max_spread_singular() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "rewardsMaxSpread": 0.05
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.rewards_max_spread, Some(0.05));
    }

    #[test]
    fn test_market_submitted_by_snake_case() {
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "submitted_by": "0xdeadbeef"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.submitted_by.as_deref(), Some("0xdeadbeef"));
    }

    // ── SeriesInfo ──────────────────────────────────────────────

    #[test]
    fn test_series_info_minimal() {
        let json = r#"{"id": "s1", "slug": "nfl-2025", "title": "NFL 2025"}"#;
        let si: SeriesInfo = serde_json::from_str(json).unwrap();
        assert_eq!(si.slug, "nfl-2025");
        assert_eq!(si.title, "NFL 2025");
        assert!(si.ticker.is_none());
        assert!(si.active.is_none());
    }

    #[test]
    fn test_series_info_full() {
        let json = r#"{
            "id": "2",
            "ticker": "nba",
            "slug": "nba",
            "title": "NBA",
            "seriesType": "single",
            "recurrence": "daily",
            "image": "https://example.com/nba.png",
            "icon": "https://example.com/nba-icon.png",
            "layout": "default",
            "active": true,
            "closed": false,
            "archived": false,
            "new": false,
            "featured": false,
            "restricted": true,
            "publishedAt": "2023-01-30T17:13:39Z",
            "createdBy": "15",
            "updatedBy": "15",
            "createdAt": "2022-10-13T00:36:01Z",
            "updatedAt": "2026-03-04T12:03:42Z",
            "commentsEnabled": false,
            "competitive": "0",
            "volume24hr": 11.07,
            "startDate": "2021-01-01T17:00:00Z",
            "commentCount": 6274,
            "requiresTranslation": false
        }"#;
        let si: SeriesInfo = serde_json::from_str(json).unwrap();
        assert_eq!(si.ticker.as_deref(), Some("nba"));
        assert_eq!(si.series_type.as_deref(), Some("single"));
        assert_eq!(si.active, Some(true));
        assert_eq!(si.closed, Some(false));
        assert_eq!(si.volume_24hr, Some(11.07));
        assert_eq!(si.comment_count, Some(6274));
        assert_eq!(si.requires_translation, Some(false));
    }

    // ── Negative tests: old/wrong field names are rejected ─────

    #[test]
    fn test_market_old_question_id_ignored() {
        // camelCase "questionId" should NOT match — API uses "questionID"
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "questionId": "0xwrong"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert!(market.question_id.is_none());
    }

    #[test]
    fn test_market_old_has_review_dates_ignored() {
        // "hasReviewDates" should NOT match — API uses "hasReviewedDates"
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "hasReviewDates": true
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert!(market.has_reviewed_dates.is_none());
    }

    #[test]
    fn test_market_old_rewards_max_spreads_ignored() {
        // "rewardsMaxSpreads" (plural) should NOT match — API uses "rewardsMaxSpread"
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "rewardsMaxSpreads": 0.05
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert!(market.rewards_max_spread.is_none());
    }

    #[test]
    fn test_event_old_enalbe_neg_risk_ignored() {
        // Old typo "enalbeNegRisk" should NOT match after fix
        let json = r#"{
            "id": "evt-1",
            "enalbeNegRisk": true
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(event.enable_neg_risk.is_none());
    }

    #[test]
    fn test_event_camel_published_at_ignored() {
        // camelCase "publishedAt" should NOT match — Event API uses "published_at"
        let json = r#"{
            "id": "evt-1",
            "publishedAt": "2024-01-01T00:00:00Z"
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(event.published_at.is_none());
    }

    #[test]
    fn test_market_camel_submitted_by_ignored() {
        // camelCase "submittedBy" should NOT match — API uses "submitted_by"
        let json = r#"{
            "id": "1",
            "conditionId": "0xcond",
            "description": "Test",
            "question": "Test?",
            "marketMakerAddress": "0xaddr",
            "submittedBy": "0xwrong"
        }"#;
        let market: Market = serde_json::from_str(json).unwrap();
        assert!(market.submitted_by.is_none());
    }

    // ── SeriesData ──────────────────────────────────────────────

    #[test]
    fn test_series_data_minimal() {
        let json = r#"{
            "id": "s1",
            "slug": "nfl",
            "title": "NFL",
            "active": true,
            "closed": false,
            "archived": false
        }"#;
        let sd: SeriesData = serde_json::from_str(json).unwrap();
        assert!(sd.active);
        assert!(!sd.closed);
        assert!(sd.events.is_empty()); // #[serde(default)]
        assert!(sd.tags.is_empty());
    }

    // ── SportMetadata ───────────────────────────────────────────

    #[test]
    fn test_sport_metadata() {
        let json = r#"{
            "id": 1,
            "sport": "Basketball",
            "image": "https://example.com/nba.png",
            "createdAt": "2024-01-01T00:00:00Z"
        }"#;
        let sm: SportMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(sm.id, 1);
        assert_eq!(sm.sport, "Basketball");
    }

    // ── Team ────────────────────────────────────────────────────

    #[test]
    fn test_team() {
        let json = r#"{
            "id": 42,
            "name": "Lakers",
            "league": "NBA",
            "abbreviation": "LAL",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-06-15T12:00:00Z"
        }"#;
        let team: Team = serde_json::from_str(json).unwrap();
        assert_eq!(team.id, 42);
        assert_eq!(team.name.as_deref(), Some("Lakers"));
        assert!(team.created_at.is_some());
    }

    // ── Comment ─────────────────────────────────────────────────

    #[test]
    fn test_comment_deserialization() {
        let json = r#"{
            "id": "c1",
            "body": "I think this market will resolve yes.",
            "createdAt": "2024-06-01T10:00:00Z",
            "updatedAt": "2024-06-01T10:00:00Z",
            "deletedAt": null,
            "user": {"id": "u1", "name": "trader1", "avatar": null},
            "marketId": "mkt-1",
            "eventId": null,
            "seriesId": null,
            "parentId": null,
            "reactions": [],
            "positions": [
                {"tokenId": "t1", "outcome": "Yes", "shares": "100.5"}
            ],
            "likeCount": 5,
            "dislikeCount": 1,
            "replyCount": 3
        }"#;
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, "c1");
        assert_eq!(comment.user.name, "trader1");
        assert_eq!(comment.like_count, 5);
        assert_eq!(comment.positions.len(), 1);
        assert_eq!(comment.positions[0].shares, "100.5");
        assert!(comment.deleted_at.is_none());
    }

    // ── UserResponse ────────────────────────────────────────────

    #[test]
    fn test_user_response() {
        let json = r#"{
            "proxyWallet": "0xproxy",
            "address": "0xsigner",
            "id": "u1",
            "name": "polytrader"
        }"#;
        let user: crate::api::user::UserResponse = serde_json::from_str(json).unwrap();
        assert_eq!(user.proxy.as_deref(), Some("0xproxy"));
        assert_eq!(user.name.as_deref(), Some("polytrader"));
    }

    #[test]
    fn test_user_response_all_null() {
        let json = r#"{}"#;
        let user: crate::api::user::UserResponse = serde_json::from_str(json).unwrap();
        assert!(user.proxy.is_none());
        assert!(user.address.is_none());
        assert!(user.id.is_none());
        assert!(user.name.is_none());
        assert!(user.created_at.is_none());
        assert!(user.profile_image.is_none());
        assert!(user.display_username_public.is_none());
        assert!(user.bio.is_none());
        assert!(user.pseudonym.is_none());
        assert!(user.x_username.is_none());
        assert!(user.verified_badge.is_none());
        assert!(user.users.is_empty());
    }

    #[test]
    fn test_user_response_full_profile() {
        let json = r#"{
            "proxyWallet": "0xproxy",
            "address": "0xsigner",
            "id": "u1",
            "name": "polytrader",
            "createdAt": "2024-01-15T10:00:00Z",
            "profileImage": "https://example.com/avatar.png",
            "displayUsernamePublic": true,
            "bio": "DeFi enthusiast",
            "pseudonym": "poly_anon",
            "xUsername": "polytrader_x",
            "verifiedBadge": true,
            "users": [
                {"id": "uid-1", "creator": true, "mod": false},
                {"id": "uid-2", "creator": false, "mod": true}
            ]
        }"#;
        let user: crate::api::user::UserResponse = serde_json::from_str(json).unwrap();
        assert_eq!(user.proxy.as_deref(), Some("0xproxy"));
        assert_eq!(user.name.as_deref(), Some("polytrader"));
        assert_eq!(user.created_at.as_deref(), Some("2024-01-15T10:00:00Z"));
        assert_eq!(
            user.profile_image.as_deref(),
            Some("https://example.com/avatar.png")
        );
        assert_eq!(user.display_username_public, Some(true));
        assert_eq!(user.bio.as_deref(), Some("DeFi enthusiast"));
        assert_eq!(user.pseudonym.as_deref(), Some("poly_anon"));
        assert_eq!(user.x_username.as_deref(), Some("polytrader_x"));
        assert_eq!(user.verified_badge, Some(true));
        assert_eq!(user.users.len(), 2);
        assert!(user.users[0].creator);
        assert!(!user.users[0].moderator);
        assert!(!user.users[1].creator);
        assert!(user.users[1].moderator);
    }

    #[test]
    fn test_user_info_deserialization() {
        let json = r#"{"id": "uid-1", "creator": true, "mod": false}"#;
        let info: crate::api::user::UserInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id.as_deref(), Some("uid-1"));
        assert!(info.creator);
        assert!(!info.moderator);
    }

    #[test]
    fn test_user_info_defaults() {
        let json = r#"{}"#;
        let info: crate::api::user::UserInfo = serde_json::from_str(json).unwrap();
        assert!(info.id.is_none());
        assert!(!info.creator);
        assert!(!info.moderator);
    }

    // ── CountResponse ────────────────────────────────────────────

    #[test]
    fn test_count_response() {
        let json = r#"{"count": 42}"#;
        let resp: CountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 42);
    }

    // ── Cursor / PaginatedResponse ──────────────────────────────

    #[test]
    fn test_cursor_with_next() {
        let json = r#"{"nextCursor": "abc123"}"#;
        let cursor: Cursor = serde_json::from_str(json).unwrap();
        assert_eq!(cursor.next_cursor.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_cursor_without_next() {
        let json = r#"{"nextCursor": null}"#;
        let cursor: Cursor = serde_json::from_str(json).unwrap();
        assert!(cursor.next_cursor.is_none());
    }

    #[test]
    fn test_paginated_response() {
        let json = r#"{
            "data": [{"tokenId": "t1", "outcome": "Yes"}],
            "nextCursor": "page2"
        }"#;
        let resp: PaginatedResponse<MarketToken> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.next_cursor.as_deref(), Some("page2"));
    }

    #[test]
    fn test_paginated_response_empty() {
        let json = r#"{"data": [], "nextCursor": null}"#;
        let resp: PaginatedResponse<MarketToken> = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    // ── Serialization round-trip ────────────────────────────────

    #[test]
    fn test_market_token_roundtrip() {
        let token = MarketToken {
            token_id: "123".into(),
            outcome: "Yes".into(),
            price: Some("0.75".into()),
            winner: Some(true),
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: MarketToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
    }

    // ── EventCreator ────────────────────────────────────────────

    #[test]
    fn test_event_creator_full() {
        let json = r#"{
            "id": "7",
            "creatorName": "Polymarket Sports",
            "creatorHandle": "poly_sports",
            "creatorUrl": "https://example.com",
            "creatorImage": "https://example.com/a.png",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-06-01T00:00:00Z"
        }"#;
        let c: EventCreator = serde_json::from_str(json).unwrap();
        assert_eq!(c.id, "7");
        assert_eq!(c.creator_name.as_deref(), Some("Polymarket Sports"));
        assert_eq!(c.creator_handle.as_deref(), Some("poly_sports"));
        assert_eq!(c.creator_url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_event_creator_minimal() {
        let json = r#"{"id": "1"}"#;
        let c: EventCreator = serde_json::from_str(json).unwrap();
        assert_eq!(c.id, "1");
        assert!(c.creator_name.is_none());
        assert!(c.creator_handle.is_none());
        assert!(c.created_at.is_none());
    }

    #[test]
    fn test_event_creator_roundtrip() {
        let c = EventCreator {
            id: "9".into(),
            creator_name: Some("Poly".into()),
            creator_handle: Some("poly".into()),
            creator_url: None,
            creator_image: None,
            created_at: None,
            updated_at: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: EventCreator = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    // ── Pagination / EventsPagination ───────────────────────────

    #[test]
    fn test_pagination_deserialization() {
        let json = r#"{"hasMore": true, "totalResults": 124}"#;
        let p: Pagination = serde_json::from_str(json).unwrap();
        assert_eq!(p.has_more, Some(true));
        assert_eq!(p.total_results, Some(124));
    }

    #[test]
    fn test_events_pagination_deserialization() {
        let json = r#"{
            "data": [{"id": "e1"}, {"id": "e2"}],
            "pagination": {"hasMore": false, "totalResults": 2}
        }"#;
        let ep: EventsPagination = serde_json::from_str(json).unwrap();
        assert_eq!(ep.data.len(), 2);
        assert_eq!(ep.data[0].id, "e1");
        let p = ep.pagination.unwrap();
        assert_eq!(p.has_more, Some(false));
        assert_eq!(p.total_results, Some(2));
    }

    #[test]
    fn test_events_pagination_empty_data() {
        let json = r#"{"pagination": {"hasMore": false, "totalResults": 0}}"#;
        let ep: EventsPagination = serde_json::from_str(json).unwrap();
        assert!(ep.data.is_empty());
    }

    // ── KeysetEventsResponse ────────────────────────────────────

    #[test]
    fn test_keyset_events_response_with_cursor() {
        // Upstream uses snake_case next_cursor (not nextCursor).
        let json = r#"{
            "events": [{"id": "e1", "title": "Test"}],
            "next_cursor": "cursor-123"
        }"#;
        let resp: KeysetEventsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.events.len(), 1);
        assert_eq!(resp.next_cursor.as_deref(), Some("cursor-123"));
    }

    #[test]
    fn test_keyset_events_response_last_page() {
        let json = r#"{"events": []}"#;
        let resp: KeysetEventsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.events.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    #[test]
    fn test_tag_roundtrip() {
        let tag = Tag {
            id: "1".into(),
            slug: "test".into(),
            label: "Test".into(),
            force_show: None,
            published_at: None,
            created_by: None,
            updated_by: None,
            created_at: None,
            updated_at: None,
            force_hide: None,
            is_carousel: None,
        };
        let json = serde_json::to_string(&tag).unwrap();
        let back: Tag = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, back);
    }

    // ── MarketDescription ───────────────────────────────────────

    #[test]
    fn test_market_description_deserialization() {
        let json = r#"{"description": "Will X happen?"}"#;
        let md: MarketDescription = serde_json::from_str(json).unwrap();
        assert_eq!(md.description.as_deref(), Some("Will X happen?"));
    }

    #[test]
    fn test_market_description_null() {
        let json = r#"{"description": null}"#;
        let md: MarketDescription = serde_json::from_str(json).unwrap();
        assert!(md.description.is_none());
    }

    // ── MarketsInformationBody ──────────────────────────────────

    #[test]
    fn test_markets_information_body_default_serializes_empty() {
        let body = MarketsInformationBody::default();
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_markets_information_body_populated_roundtrip() {
        let body = MarketsInformationBody {
            id: vec![1, 2, 3],
            slug: vec!["will-x".into(), "will-y".into()],
            closed: Some(true),
            clob_token_ids: vec!["tok-a".into()],
            condition_ids: vec!["0xcond".into()],
            market_maker_address: vec!["0xmm".into()],
            liquidity_num_min: Some(100.0),
            liquidity_num_max: Some(10_000.0),
            volume_num_min: Some(50.0),
            volume_num_max: None,
            start_date_min: Some("2025-01-01T00:00:00Z".into()),
            start_date_max: None,
            end_date_min: None,
            end_date_max: Some("2026-01-01T00:00:00Z".into()),
            related_tags: Some(true),
            tag_id: Some(42),
            cyom: Some(false),
            uma_resolution_status: Some("resolved".into()),
            game_id: Some("game-7".into()),
            sports_market_types: vec!["moneyline".into(), "spread".into()],
            rewards_min_size: Some(10.0),
            question_ids: vec!["q1".into()],
            include_tags: Some(true),
        };
        let json = serde_json::to_string(&body).unwrap();
        // Field names are camelCase (from openapi)
        assert!(json.contains("\"clobTokenIds\""));
        assert!(json.contains("\"marketMakerAddress\""));
        assert!(json.contains("\"rewardsMinSize\""));
        let back: MarketsInformationBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, back);
    }

    #[test]
    fn test_markets_information_body_partial_omits_empty() {
        let body = MarketsInformationBody {
            closed: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"closed":true}"#);
    }

    // ── KeysetMarketsResponse ───────────────────────────────────

    #[test]
    fn test_keyset_markets_response_full() {
        let json = r#"{
            "markets": [{
                "id": "1",
                "conditionId": "0xcond",
                "description": "desc",
                "question": "q?",
                "marketMakerAddress": "0xmm"
            }],
            "next_cursor": "abc"
        }"#;
        let resp: KeysetMarketsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.markets.len(), 1);
        assert_eq!(resp.markets[0].id, "1");
        assert_eq!(resp.next_cursor.as_deref(), Some("abc"));
    }

    #[test]
    fn test_keyset_markets_response_last_page() {
        let json = r#"{"markets": []}"#;
        let resp: KeysetMarketsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.markets.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    // ── SeriesSummary ───────────────────────────────────────────

    #[test]
    fn test_series_summary_full() {
        let json = r#"{
            "id": "s-1",
            "title": "NFL 2025",
            "slug": "nfl-2025",
            "eventDates": ["2025-09-01", "2025-09-08"],
            "eventWeeks": [1, 2],
            "earliest_open_week": 1,
            "earliest_open_date": "2025-09-01"
        }"#;
        let s: SeriesSummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.id, "s-1");
        assert_eq!(s.title.as_deref(), Some("NFL 2025"));
        assert_eq!(s.event_dates.len(), 2);
        assert_eq!(s.event_weeks, vec![1, 2]);
        assert_eq!(s.earliest_open_week, Some(1));
    }

    #[test]
    fn test_series_summary_minimal() {
        let json = r#"{"id": "s-1"}"#;
        let s: SeriesSummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.id, "s-1");
        assert!(s.title.is_none());
        assert!(s.event_dates.is_empty());
        assert!(s.event_weeks.is_empty());
    }

    #[test]
    fn test_series_summary_roundtrip_preserves_mixed_casing() {
        let s = SeriesSummary {
            id: "s-1".into(),
            title: Some("T".into()),
            slug: Some("slug".into()),
            event_dates: vec!["2025-09-01".into()],
            event_weeks: vec![1],
            earliest_open_week: Some(1),
            earliest_open_date: Some("2025-09-01".into()),
        };
        let json = serde_json::to_string(&s).unwrap();
        // eventDates camelCase, earliest_open_* snake_case
        assert!(json.contains("\"eventDates\""));
        assert!(json.contains("\"eventWeeks\""));
        assert!(json.contains("\"earliest_open_week\""));
        assert!(json.contains("\"earliest_open_date\""));
        let back: SeriesSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // ── Profile ─────────────────────────────────────────────────

    #[test]
    fn test_profile_minimal() {
        let json = r#"{"id": "p-1"}"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.id, "p-1");
        assert!(p.name.is_none());
        assert!(p.proxy_wallet.is_none());
    }

    #[test]
    fn test_profile_full() {
        let json = r#"{
            "id": "p-1",
            "name": "Alice",
            "user": 42,
            "proxyWallet": "0xdead",
            "utmSource": "direct",
            "walletActivated": true,
            "isCloseOnly": false,
            "profileImageOptimized": {"medium": "https://cdn/p/m.png"}
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.name.as_deref(), Some("Alice"));
        assert_eq!(p.user, Some(42));
        assert_eq!(p.proxy_wallet.as_deref(), Some("0xdead"));
        assert_eq!(p.utm_source.as_deref(), Some("direct"));
        assert_eq!(p.wallet_activated, Some(true));
        assert_eq!(p.is_close_only, Some(false));
        assert!(p.profile_image_optimized.is_some());
    }

    #[test]
    fn test_profile_roundtrip() {
        let p = Profile {
            id: "p-1".into(),
            name: Some("A".into()),
            user: Some(1),
            referral: None,
            created_by: None,
            updated_by: None,
            created_at: None,
            updated_at: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            utm_content: None,
            utm_term: None,
            wallet_activated: Some(true),
            pseudonym: None,
            display_username_public: None,
            profile_image: None,
            bio: None,
            proxy_wallet: Some("0xpw".into()),
            profile_image_optimized: None,
            is_close_only: None,
            is_cert_req: None,
            cert_req_date: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"proxyWallet\""));
        assert!(json.contains("\"walletActivated\""));
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
