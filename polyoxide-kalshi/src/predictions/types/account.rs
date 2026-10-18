//! The account routes that need no key.

use serde::{Deserialize, Serialize};

/// One route whose token cost differs from the default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointCost {
    /// The HTTP method.
    pub method: String,
    /// The route's full template, such as `/trade-api/v2/markets/:ticker`.
    pub path: String,
    /// Its cost in tokens.
    pub cost: u32,
}

/// `GET /account/endpoint_costs`, which answers without a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointCosts {
    /// What a route not listed costs, in tokens.
    pub default_cost: u32,
    /// The routes whose cost differs from the default.
    pub endpoint_costs: Vec<EndpointCost>,
}
