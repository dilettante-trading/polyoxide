use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::GammaError,
    types::{Market, Tag},
};

/// Markets namespace for market-related operations
#[derive(Clone)]
pub struct Markets {
    pub(crate) http_client: HttpClient,
}

impl Markets {
    /// Get a specific market by ID
    pub fn get(&self, id: impl Into<String>) -> GetMarket {
        GetMarket {
            request: Request::new(
                self.http_client.clone(),
                format!("/markets/{}", urlencoding::encode(&id.into())),
            ),
        }
    }

    /// Get a market by its slug
    pub fn get_by_slug(&self, slug: impl Into<String>) -> GetMarket {
        GetMarket {
            request: Request::new(
                self.http_client.clone(),
                format!("/markets/slug/{}", urlencoding::encode(&slug.into())),
            ),
        }
    }

    /// List markets with optional filtering
    pub fn list(&self) -> ListMarkets {
        ListMarkets {
            request: Request::new(self.http_client.clone(), "/markets"),
        }
    }

    /// Get tags for a market
    pub fn tags(&self, id: impl Into<String>) -> Request<Vec<Tag>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/markets/{}/tags", urlencoding::encode(&id.into())),
        )
    }
}

/// Request builder for getting a single market
pub struct GetMarket {
    request: Request<Market, GammaError>,
}

impl GetMarket {
    /// Include tag data in response
    pub fn include_tag(mut self, include: bool) -> Self {
        self.request = self.request.query("include_tag", include);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Market, GammaError> {
        self.request.send().await
    }
}

/// Request builder for listing markets
pub struct ListMarkets {
    request: Request<Vec<Market>, GammaError>,
}

impl ListMarkets {
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

    /// Filter by specific market IDs
    ///
    /// Safe batch size: ≤ 400 per request. URLs over ~8 KB are rejected
    /// upstream with `414 URI Too Long`; empirically the ceiling is ~583.
    pub fn id(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.request = self.request.query_many("id", ids);
        self
    }

    /// Filter by market slugs
    ///
    /// Safe batch size: ≤ 100 per request. URL length is capped at ~8 KB
    /// upstream; slug entries vary so pick a cap based on your longest slug.
    pub fn slug(mut self, slugs: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("slug", slugs);
        self
    }

    /// Filter by CLOB token IDs
    ///
    /// Safe batch size: ≤ 50 per request. Token IDs are 77-digit decimals
    /// (~90 B/entry on the wire); URLs over ~8 KB are rejected with `414`.
    pub fn clob_token_ids(mut self, token_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("clob_token_ids", token_ids);
        self
    }

    /// Filter by condition IDs
    ///
    /// Safe batch size: ≤ 60 per request. Condition IDs are 66-char hex
    /// (~80 B/entry); empirically the upstream ceiling is exactly 100 before
    /// `414 URI Too Long`.
    pub fn condition_ids(mut self, condition_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("condition_ids", condition_ids);
        self
    }

    /// Filter by market maker addresses
    ///
    /// Safe batch size: ≤ 80 per request. Ethereum addresses are 42 chars
    /// (~60 B/entry); URLs over ~8 KB are rejected upstream with `414`.
    pub fn market_maker_address(
        mut self,
        addresses: impl IntoIterator<Item = impl ToString>,
    ) -> Self {
        self.request = self.request.query_many("market_maker_address", addresses);
        self
    }

    /// Set minimum liquidity threshold
    pub fn liquidity_num_min(mut self, min: f64) -> Self {
        self.request = self.request.query("liquidity_num_min", min);
        self
    }

    /// Set maximum liquidity threshold
    pub fn liquidity_num_max(mut self, max: f64) -> Self {
        self.request = self.request.query("liquidity_num_max", max);
        self
    }

    /// Set minimum trading volume
    pub fn volume_num_min(mut self, min: f64) -> Self {
        self.request = self.request.query("volume_num_min", min);
        self
    }

    /// Set maximum trading volume
    pub fn volume_num_max(mut self, max: f64) -> Self {
        self.request = self.request.query("volume_num_max", max);
        self
    }

    /// Set earliest market start date (ISO 8601 format)
    pub fn start_date_min(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("start_date_min", date.into());
        self
    }

    /// Set latest market start date (ISO 8601 format)
    pub fn start_date_max(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("start_date_max", date.into());
        self
    }

    /// Set earliest market end date (ISO 8601 format)
    pub fn end_date_min(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("end_date_min", date.into());
        self
    }

    /// Set latest market end date (ISO 8601 format)
    pub fn end_date_max(mut self, date: impl Into<String>) -> Self {
        self.request = self.request.query("end_date_max", date.into());
        self
    }

    /// Filter by tag identifier
    pub fn tag_id(mut self, tag_id: i64) -> Self {
        self.request = self.request.query("tag_id", tag_id);
        self
    }

    /// Include related tags in response
    pub fn related_tags(mut self, include: bool) -> Self {
        self.request = self.request.query("related_tags", include);
        self
    }

    /// Filter for create-your-own markets
    pub fn cyom(mut self, cyom: bool) -> Self {
        self.request = self.request.query("cyom", cyom);
        self
    }

    /// Filter by UMA resolution status
    pub fn uma_resolution_status(mut self, status: impl Into<String>) -> Self {
        self.request = self.request.query("uma_resolution_status", status.into());
        self
    }

    /// Filter by game identifier
    pub fn game_id(mut self, game_id: impl Into<String>) -> Self {
        self.request = self.request.query("game_id", game_id.into());
        self
    }

    /// Filter by sports market types
    ///
    /// Safe batch size: ≤ 150 per request. URL length is capped at ~8 KB
    /// upstream (`414 URI Too Long`).
    pub fn sports_market_types(mut self, types: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("sports_market_types", types);
        self
    }

    /// Set minimum rewards threshold
    pub fn rewards_min_size(mut self, min: f64) -> Self {
        self.request = self.request.query("rewards_min_size", min);
        self
    }

    /// Filter by question identifiers
    ///
    /// Safe batch size: ≤ 60 per request. Question IDs are 66-char hex
    /// (~80 B/entry); URLs over ~8 KB are rejected upstream with `414`.
    pub fn question_ids(mut self, question_ids: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("question_ids", question_ids);
        self
    }

    /// Include tag data in results
    pub fn include_tag(mut self, include: bool) -> Self {
        self.request = self.request.query("include_tag", include);
        self
    }

    /// Filter for closed or active markets
    pub fn closed(mut self, closed: bool) -> Self {
        self.request = self.request.query("closed", closed);
        self
    }

    /// Filter by open status (convenience method, opposite of closed)
    pub fn open(mut self, open: bool) -> Self {
        self.request = self.request.query("closed", !open);
        self
    }

    /// Filter by archived status
    pub fn archived(mut self, archived: bool) -> Self {
        self.request = self.request.query("archived", archived);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Market>, GammaError> {
        self.request.send().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    /// Verify that all builder methods chain correctly (compile-time type check)
    /// and produce a valid builder ready to send.
    #[test]
    fn test_list_markets_full_chain() {
        // This test verifies that every builder method returns Self and chains
        let _list = gamma()
            .markets()
            .list()
            .limit(25)
            .offset(50)
            .order("volume")
            .ascending(false)
            .id(vec![1i64, 2, 3])
            .slug(vec!["slug-a"])
            .clob_token_ids(vec!["token-1"])
            .condition_ids(vec!["cond-1"])
            .market_maker_address(vec!["0xaddr"])
            .liquidity_num_min(1000.0)
            .liquidity_num_max(50000.0)
            .volume_num_min(100.0)
            .volume_num_max(10000.0)
            .start_date_min("2024-01-01")
            .start_date_max("2025-01-01")
            .end_date_min("2024-06-01")
            .end_date_max("2025-12-31")
            .tag_id(42)
            .related_tags(true)
            .cyom(false)
            .uma_resolution_status("resolved")
            .game_id("game-1")
            .sports_market_types(vec!["moneyline"])
            .rewards_min_size(10.0)
            .question_ids(vec!["q1"])
            .include_tag(true)
            .closed(false)
            .archived(false);
    }

    #[test]
    fn test_open_and_closed_are_inverse() {
        // Both should compile and produce a valid builder
        let _open = gamma().markets().list().open(true);
        let _closed = gamma().markets().list().closed(false);
    }

    #[test]
    fn test_get_market_accepts_string_and_str() {
        let _req1 = gamma().markets().get("12345");
        let _req2 = gamma().markets().get(String::from("12345"));
    }

    #[test]
    fn test_get_by_slug_accepts_string_and_str() {
        let _req1 = gamma().markets().get_by_slug("my-slug");
        let _req2 = gamma().markets().get_by_slug(String::from("my-slug"));
    }

    #[test]
    fn test_get_market_with_include_tag() {
        let _req = gamma().markets().get("12345").include_tag(true);
    }

    #[test]
    fn test_market_tags_accepts_str_and_string() {
        let _req1 = gamma().markets().tags("12345");
        let _req2 = gamma().markets().tags(String::from("12345"));
    }
}
