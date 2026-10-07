//! Reference data: `/fapi/v1/exchangeInfo` and `/fapi/v1/fundingInfo`.

use polyoxide_core::HttpClient;

use crate::{
    usdm::{
        request::WeightedRequest,
        types::{ExchangeInfo, FundingInfo},
    },
    weight::{Route, WeightBudget},
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) http: HttpClient,
    pub(crate) budget: WeightBudget,
}

impl ExchangeApi {
    /// `GET /fapi/v1/exchangeInfo` (weight 1): every contract, its filters, and
    /// the IP's limits.
    pub fn exchange_info(&self) -> WeightedRequest<ExchangeInfo> {
        WeightedRequest::new(&self.http, &self.budget, Route::ExchangeInfo)
    }

    /// `GET /fapi/v1/fundingInfo`: the symbols whose funding cap, floor or
    /// interval was adjusted. Paced by the funding limit, not by weight.
    pub fn funding_info(&self) -> WeightedRequest<Vec<FundingInfo>> {
        WeightedRequest::new(&self.http, &self.budget, Route::FundingInfo)
    }
}
