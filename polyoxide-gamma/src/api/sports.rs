use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::GammaError,
    types::{SportMetadata, Team},
};

/// Sport namespace for sports-related operations
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
    pub fn league(mut self, leagues: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("league", leagues);
        self
    }

    /// Filter by team name(s)
    pub fn name(mut self, names: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("name", names);
        self
    }

    /// Filter by team abbreviation(s)
    pub fn abbreviation(mut self, abbreviations: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("abbreviation", abbreviations);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Team>, GammaError> {
        self.request.send().await
    }
}
