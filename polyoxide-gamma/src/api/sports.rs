use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::GammaError,
    types::{SportMetadata, Team},
};

/// Sports namespace for sports-related operations
#[derive(Clone)]
pub struct Sports {
    pub(crate) http_client: HttpClient,
}

impl Sports {
    /// Get all sports metadata
    pub fn list(&self) -> Request<Vec<SportMetadata>, GammaError> {
        Request::new(self.http_client.clone(), "/sports")
    }

    /// Get available sports market types
    pub fn market_types(&self) -> Request<serde_json::Value, GammaError> {
        Request::new(self.http_client.clone(), "/sports/market-types")
    }

    /// List teams with optional filtering
    pub fn list_teams(&self) -> ListTeams {
        ListTeams {
            request: Request::new(self.http_client.clone(), "/teams"),
        }
    }

    /// Get a team by ID (`GET /teams/{id}`).
    pub fn get_team(&self, id: impl Into<String>) -> Request<Team, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/teams/{}", urlencoding::encode(&id.into())),
        )
    }
}

/// Request builder for listing teams
pub struct ListTeams {
    request: Request<Vec<Team>, GammaError>,
}

impl ListTeams {
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

    /// Filter by league identifier(s)
    ///
    /// Safe batch size: ≤ 300 per request. URLs over ~8 KB are rejected
    /// upstream with `414 URI Too Long`.
    pub fn league(mut self, leagues: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("league", leagues);
        self
    }

    /// Filter by team name(s)
    ///
    /// Safe batch size: ≤ 200 per request. URL length is capped at ~8 KB
    /// upstream; name entries vary so pick a cap based on your longest name.
    pub fn name(mut self, names: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("name", names);
        self
    }

    /// Filter by team abbreviation(s)
    ///
    /// Safe batch size: ≤ 300 per request. Abbreviations are short
    /// (~3-5 B/entry); URLs over ~8 KB are rejected upstream with `414`.
    pub fn abbreviation(mut self, abbreviations: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("abbreviation", abbreviations);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Team>, GammaError> {
        self.request.send().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    #[test]
    fn test_get_team_accepts_str_and_string() {
        let _r1 = gamma().sports().get_team("12345");
        let _r2 = gamma().sports().get_team(String::from("12345"));
    }
}
