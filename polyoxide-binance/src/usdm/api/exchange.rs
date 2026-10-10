//! Reference data: `/fapi/v1/exchangeInfo` and `/fapi/v1/fundingInfo`.

use polyoxide_core::HttpClient;

use crate::{
    usdm::{
        request::{route_builder, Routed},
        types::{ExchangeInfo, FundingInfo},
    },
    weight::Route,
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) http: HttpClient,
}

impl ExchangeApi {
    /// `GET /fapi/v1/exchangeInfo` (weight 1): every contract, its filters, and
    /// the IP's limits.
    pub fn exchange_info(&self) -> GetExchangeInfo {
        GetExchangeInfo {
            request: Routed::new(&self.http, Route::ExchangeInfo),
        }
    }

    /// `GET /fapi/v1/fundingInfo`: the symbols whose funding cap, floor or
    /// interval was adjusted. Paced by the funding limit, not by weight.
    pub fn funding_info(&self) -> GetFundingInfo {
        GetFundingInfo {
            request: Routed::new(&self.http, Route::FundingInfo),
        }
    }
}

route_builder! {
    /// Request builder for `GET /fapi/v1/exchangeInfo`.
    GetExchangeInfo => ExchangeInfo
}

route_builder! {
    /// Request builder for `GET /fapi/v1/fundingInfo`.
    GetFundingInfo => Vec<FundingInfo>
}
