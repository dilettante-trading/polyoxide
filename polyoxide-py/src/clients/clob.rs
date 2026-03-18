use std::sync::Arc;
use pyo3::prelude::*;
use pyo3::types::PyModuleMethods;

use crate::error::clob_err;
use crate::types::*;

fn parse_order_side(s: &str) -> PyResult<polyoxide_clob::types::OrderSide> {
    use polyoxide_clob::types::OrderSide;
    parse_enum!(s, OrderSide, Buy => "BUY", Sell => "SELL")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Markets
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyClobMarkets,
    sync_name = PyClobMarketsSync,
    py_async_name = "ClobMarkets",
    py_sync_name = "ClobMarketsSync",
    client_type = polyoxide_clob::Clob,
    client_var = client,

    #[pyo3(signature = (condition_id,))]
    fn get(condition_id: String) -> PyClobMarket {
        Ok(PyClobMarket::from(client.markets().get(condition_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_ids,))]
    fn get_by_token_ids(token_ids: Vec<String>) -> PyListMarketsResponse {
        Ok(PyListMarketsResponse::from(client.markets().get_by_token_ids(token_ids).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = ())]
    fn list() -> PyListMarketsResponse {
        Ok(PyListMarketsResponse::from(client.markets().list().send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn order_book(token_id: String) -> PyOrderBook {
        Ok(PyOrderBook::from(client.markets().order_book(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id, side))]
    fn price(token_id: String, side: String) -> PyPriceResponse {
        let side = parse_order_side(&side)?;
        Ok(PyPriceResponse::from(client.markets().price(token_id, side).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn midpoint(token_id: String) -> PyMidpointResponse {
        Ok(PyMidpointResponse::from(client.markets().midpoint(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn prices_history(token_id: String) -> PyPricesHistoryResponse {
        Ok(PyPricesHistoryResponse::from(client.markets().prices_history(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn neg_risk(token_id: String) -> PyNegRiskResponse {
        Ok(PyNegRiskResponse::from(client.markets().neg_risk(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn fee_rate(token_id: String) -> PyFeeRateResponse {
        Ok(PyFeeRateResponse::from(client.markets().fee_rate(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn tick_size(token_id: String) -> PyTickSizeResponse {
        Ok(PyTickSizeResponse::from(client.markets().tick_size(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn spread(token_id: String) -> PySpreadResponse {
        Ok(PySpreadResponse::from(client.markets().spread(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id,))]
    fn last_trade_price(token_id: String) -> PyLastTradePriceResponse {
        Ok(PyLastTradePriceResponse::from(client.markets().last_trade_price(token_id).send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (condition_id,))]
    fn live_activity(condition_id: String) -> Vec<PyLiveActivityEvent> {
        let result = client.markets().live_activity(condition_id).send().await.map_err(clob_err)?;
        Ok(result.into_iter().map(PyLiveActivityEvent::from).collect::<Vec<_>>())
    }

    #[pyo3(signature = ())]
    fn simplified() -> PyListMarketsResponse {
        Ok(PyListMarketsResponse::from(client.markets().simplified().send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = ())]
    fn sampling() -> PyListMarketsResponse {
        Ok(PyListMarketsResponse::from(client.markets().sampling().send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = ())]
    fn sampling_simplified() -> PyListMarketsResponse {
        Ok(PyListMarketsResponse::from(client.markets().sampling_simplified().send().await.map_err(clob_err)?))
    }

    #[pyo3(signature = (token_id, side, amount))]
    fn calculate_price(token_id: String, side: String, amount: String) -> PyCalculatePriceResponse {
        let side = parse_order_side(&side)?;
        Ok(PyCalculatePriceResponse::from(client.markets().calculate_price(token_id, side, amount).await.map_err(clob_err)?))
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Health
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyClobHealth,
    sync_name = PyClobHealthSync,
    py_async_name = "ClobHealth",
    py_sync_name = "ClobHealthSync",
    client_type = polyoxide_clob::Clob,
    client_var = client,

    #[pyo3(signature = ())]
    fn ping() -> f64 {
        let duration = client.health().ping().await.map_err(clob_err)?;
        Ok(duration.as_secs_f64())
    }

    #[pyo3(signature = ())]
    fn server_time() -> PyServerTimeResponse {
        Ok(PyServerTimeResponse::from(client.health().server_time().send().await.map_err(clob_err)?))
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Async Client
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "ClobClient", skip_from_py_object)]
pub struct PyClobClient {
    client: Arc<polyoxide_clob::Clob>,
}

#[pymethods]
impl PyClobClient {
    #[new]
    #[pyo3(signature = ())]
    fn new() -> Self {
        Self { client: Arc::new(polyoxide_clob::Clob::public()) }
    }

    fn markets(&self) -> PyClobMarkets {
        PyClobMarkets { client: self.client.clone() }
    }

    fn health(&self) -> PyClobHealth {
        PyClobHealth { client: self.client.clone() }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sync Client
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "ClobClientSync", skip_from_py_object)]
pub struct PyClobClientSync {
    client: Arc<polyoxide_clob::Clob>,
}

#[pymethods]
impl PyClobClientSync {
    #[new]
    #[pyo3(signature = ())]
    fn new() -> Self {
        Self { client: Arc::new(polyoxide_clob::Clob::public()) }
    }

    fn markets(&self) -> PyClobMarketsSync {
        PyClobMarketsSync { client: self.client.clone() }
    }

    fn health(&self) -> PyClobHealthSync {
        PyClobHealthSync { client: self.client.clone() }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Registration
// ═══════════════════════════════════════════════════════════════════════════════

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyClobClient>()?;
    m.add_class::<PyClobClientSync>()?;
    m.add_class::<PyClobMarkets>()?;
    m.add_class::<PyClobMarketsSync>()?;
    m.add_class::<PyClobHealth>()?;
    m.add_class::<PyClobHealthSync>()?;
    Ok(())
}
