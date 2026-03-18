use pyo3::prelude::*;
use pyo3::types::PyModuleMethods;
use std::sync::Arc;

use crate::error::data_err;
use crate::types::*;

// ═══════════════════════════════════════════════════════════════════════════════
// Enum parsing helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn parse_position_sort_by(s: &str) -> PyResult<polyoxide_data::types::PositionSortBy> {
    use polyoxide_data::types::PositionSortBy;
    parse_enum!(s, PositionSortBy,
        Current => "CURRENT", Initial => "INITIAL", Tokens => "TOKENS",
        CashPnl => "CASH_PNL", PercentPnl => "PERCENT_PNL", Title => "TITLE",
        Resolving => "RESOLVING", Price => "PRICE", AvgPrice => "AVG_PRICE",
    )
}

fn parse_sort_direction(s: &str) -> PyResult<polyoxide_data::types::SortDirection> {
    use polyoxide_data::types::SortDirection;
    parse_enum!(s, SortDirection, Asc => "ASC", Desc => "DESC")
}

fn parse_closed_position_sort_by(s: &str) -> PyResult<polyoxide_data::types::ClosedPositionSortBy> {
    use polyoxide_data::types::ClosedPositionSortBy;
    parse_enum!(s, ClosedPositionSortBy,
        RealizedPnl => "REALIZED_PNL", Title => "TITLE", Price => "PRICE",
        AvgPrice => "AVG_PRICE", Timestamp => "TIMESTAMP",
    )
}

fn parse_trade_side(s: &str) -> PyResult<polyoxide_data::types::TradeSide> {
    use polyoxide_data::types::TradeSide;
    parse_enum!(s, TradeSide, Buy => "BUY", Sell => "SELL")
}

fn parse_trade_filter_type(s: &str) -> PyResult<polyoxide_data::types::TradeFilterType> {
    use polyoxide_data::types::TradeFilterType;
    parse_enum!(s, TradeFilterType, Cash => "CASH", Tokens => "TOKENS")
}

fn parse_activity_type(s: &str) -> PyResult<polyoxide_data::types::ActivityType> {
    use polyoxide_data::types::ActivityType;
    parse_enum!(s, ActivityType,
        Trade => "TRADE", Split => "SPLIT", Merge => "MERGE",
        Redeem => "REDEEM", Reward => "REWARD", Conversion => "CONVERSION",
    )
}

fn parse_activity_sort_by(s: &str) -> PyResult<polyoxide_data::types::ActivitySortBy> {
    use polyoxide_data::types::ActivitySortBy;
    parse_enum!(s, ActivitySortBy, Timestamp => "TIMESTAMP", Tokens => "TOKENS", Cash => "CASH")
}

fn parse_time_period(s: &str) -> PyResult<polyoxide_data::types::TimePeriod> {
    use polyoxide_data::types::TimePeriod;
    parse_enum!(s, TimePeriod, Day => "DAY", Week => "WEEK", Month => "MONTH", All => "ALL")
}

fn parse_leaderboard_category(
    s: &str,
) -> PyResult<polyoxide_data::api::leaderboard::LeaderboardCategory> {
    use polyoxide_data::api::leaderboard::LeaderboardCategory;
    parse_enum!(s, LeaderboardCategory,
        Overall => "OVERALL", Politics => "POLITICS", Sports => "SPORTS",
        Crypto => "CRYPTO", Culture => "CULTURE", Mentions => "MENTIONS",
        Weather => "WEATHER", Economics => "ECONOMICS", Tech => "TECH", Finance => "FINANCE",
    )
}

fn parse_leaderboard_order_by(
    s: &str,
) -> PyResult<polyoxide_data::api::leaderboard::LeaderboardOrderBy> {
    use polyoxide_data::api::leaderboard::LeaderboardOrderBy;
    parse_enum!(s, LeaderboardOrderBy, Pnl => "PNL", Vol => "VOL")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: User (manual — holds address state)
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "DataApiUser", skip_from_py_object)]
pub struct PyDataApiUser {
    client: Arc<polyoxide_data::DataApi>,
    address: String,
}

#[pymethods]
impl PyDataApiUser {
    #[pyo3(signature = (*, market=None, event_id=None, size_threshold=None, redeemable=None, mergeable=None, limit=None, offset=None, sort_by=None, sort_direction=None, title=None))]
    #[allow(clippy::too_many_arguments)]
    fn list_positions<'py>(
        &self,
        py: Python<'py>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        size_threshold: Option<f64>,
        redeemable: Option<bool>,
        mergeable: Option<bool>,
        limit: Option<u32>,
        offset: Option<u32>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        title: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let sort_by = sort_by.map(|s| parse_position_sort_by(&s)).transpose()?;
        let sort_direction = sort_direction
            .map(|s| parse_sort_direction(&s))
            .transpose()?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut req = client.user(&address).list_positions();
            if let Some(v) = market {
                req = req.market(v);
            }
            if let Some(v) = event_id {
                req = req.event_id(v);
            }
            if let Some(v) = size_threshold {
                req = req.size_threshold(v);
            }
            if let Some(v) = redeemable {
                req = req.redeemable(v);
            }
            if let Some(v) = mergeable {
                req = req.mergeable(v);
            }
            if let Some(v) = limit {
                req = req.limit(v);
            }
            if let Some(v) = offset {
                req = req.offset(v);
            }
            if let Some(v) = sort_by {
                req = req.sort_by(v);
            }
            if let Some(v) = sort_direction {
                req = req.sort_direction(v);
            }
            if let Some(v) = title {
                req = req.title(v);
            }
            let result = req.send().await.map_err(data_err)?;
            Ok(result.into_iter().map(PyPosition::from).collect::<Vec<_>>())
        })
    }

    #[pyo3(signature = (*, market=None))]
    fn positions_value<'py>(
        &self,
        py: Python<'py>,
        market: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let address = self.address.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut req = client.user(&address).positions_value();
            if let Some(v) = market {
                req = req.market(v);
            }
            let result = req.send().await.map_err(data_err)?;
            Ok(result
                .into_iter()
                .map(PyUserValue::from)
                .collect::<Vec<_>>())
        })
    }

    #[pyo3(signature = (*, market=None, event_id=None, title=None, limit=None, offset=None, sort_by=None, sort_direction=None))]
    #[allow(clippy::too_many_arguments)]
    fn closed_positions<'py>(
        &self,
        py: Python<'py>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        title: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let sort_by = sort_by
            .map(|s| parse_closed_position_sort_by(&s))
            .transpose()?;
        let sort_direction = sort_direction
            .map(|s| parse_sort_direction(&s))
            .transpose()?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut req = client.user(&address).closed_positions();
            if let Some(v) = market {
                req = req.market(v);
            }
            if let Some(v) = event_id {
                req = req.event_id(v);
            }
            if let Some(v) = title {
                req = req.title(v);
            }
            if let Some(v) = limit {
                req = req.limit(v);
            }
            if let Some(v) = offset {
                req = req.offset(v);
            }
            if let Some(v) = sort_by {
                req = req.sort_by(v);
            }
            if let Some(v) = sort_direction {
                req = req.sort_direction(v);
            }
            let result = req.send().await.map_err(data_err)?;
            Ok(result
                .into_iter()
                .map(PyClosedPosition::from)
                .collect::<Vec<_>>())
        })
    }

    #[pyo3(signature = (*, market=None, event_id=None, side=None, taker_only=None, filter_type=None, filter_amount=None, limit=None, offset=None))]
    #[allow(clippy::too_many_arguments)]
    fn trades<'py>(
        &self,
        py: Python<'py>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        side: Option<String>,
        taker_only: Option<bool>,
        filter_type: Option<String>,
        filter_amount: Option<f64>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let side = side.map(|s| parse_trade_side(&s)).transpose()?;
        let filter_type = filter_type
            .map(|s| parse_trade_filter_type(&s))
            .transpose()?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut req = client.user(&address).trades();
            if let Some(v) = market {
                req = req.market(v);
            }
            if let Some(v) = event_id {
                req = req.event_id(v);
            }
            if let Some(v) = side {
                req = req.side(v);
            }
            if let Some(v) = taker_only {
                req = req.taker_only(v);
            }
            if let Some(v) = filter_type {
                req = req.filter_type(v);
            }
            if let Some(v) = filter_amount {
                req = req.filter_amount(v);
            }
            if let Some(v) = limit {
                req = req.limit(v);
            }
            if let Some(v) = offset {
                req = req.offset(v);
            }
            let result = req.send().await.map_err(data_err)?;
            Ok(result.into_iter().map(PyTrade::from).collect::<Vec<_>>())
        })
    }

    #[pyo3(signature = (*, market=None, event_id=None, activity_type=None, side=None, start=None, end=None, limit=None, offset=None, sort_by=None, sort_direction=None))]
    #[allow(clippy::too_many_arguments)]
    fn activity<'py>(
        &self,
        py: Python<'py>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        activity_type: Option<Vec<String>>,
        side: Option<String>,
        start: Option<i64>,
        end: Option<i64>,
        limit: Option<u32>,
        offset: Option<u32>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let activity_type = activity_type
            .map(|v| {
                v.iter()
                    .map(|s| parse_activity_type(s))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let side = side.map(|s| parse_trade_side(&s)).transpose()?;
        let sort_by = sort_by.map(|s| parse_activity_sort_by(&s)).transpose()?;
        let sort_direction = sort_direction
            .map(|s| parse_sort_direction(&s))
            .transpose()?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut req = client.user(&address).activity();
            if let Some(v) = market {
                req = req.market(v);
            }
            if let Some(v) = event_id {
                req = req.event_id(v);
            }
            if let Some(v) = activity_type {
                req = req.activity_type(v);
            }
            if let Some(v) = side {
                req = req.side(v);
            }
            if let Some(v) = start {
                req = req.start(v);
            }
            if let Some(v) = end {
                req = req.end(v);
            }
            if let Some(v) = limit {
                req = req.limit(v);
            }
            if let Some(v) = offset {
                req = req.offset(v);
            }
            if let Some(v) = sort_by {
                req = req.sort_by(v);
            }
            if let Some(v) = sort_direction {
                req = req.sort_direction(v);
            }
            let result = req.send().await.map_err(data_err)?;
            Ok(result.into_iter().map(PyActivity::from).collect::<Vec<_>>())
        })
    }

    #[pyo3(signature = ())]
    fn traded<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let address = self.address.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            Ok(PyUserTraded::from(
                client.user(&address).traded().await.map_err(data_err)?,
            ))
        })
    }
}

// Sync variant
#[pyclass(name = "DataApiUserSync", skip_from_py_object)]
pub struct PyDataApiUserSync {
    client: Arc<polyoxide_data::DataApi>,
    address: String,
}

#[pymethods]
impl PyDataApiUserSync {
    #[pyo3(signature = (*, market=None, event_id=None, size_threshold=None, redeemable=None, mergeable=None, limit=None, offset=None, sort_by=None, sort_direction=None, title=None))]
    #[allow(clippy::too_many_arguments)]
    fn list_positions(
        &self,
        py: Python<'_>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        size_threshold: Option<f64>,
        redeemable: Option<bool>,
        mergeable: Option<bool>,
        limit: Option<u32>,
        offset: Option<u32>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        title: Option<String>,
    ) -> PyResult<Vec<PyPosition>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let sort_by = sort_by.map(|s| parse_position_sort_by(&s)).transpose()?;
        let sort_direction = sort_direction
            .map(|s| parse_sort_direction(&s))
            .transpose()?;
        py.detach(|| {
            crate::runtime::runtime().block_on(async move {
                let mut req = client.user(&address).list_positions();
                if let Some(v) = market {
                    req = req.market(v);
                }
                if let Some(v) = event_id {
                    req = req.event_id(v);
                }
                if let Some(v) = size_threshold {
                    req = req.size_threshold(v);
                }
                if let Some(v) = redeemable {
                    req = req.redeemable(v);
                }
                if let Some(v) = mergeable {
                    req = req.mergeable(v);
                }
                if let Some(v) = limit {
                    req = req.limit(v);
                }
                if let Some(v) = offset {
                    req = req.offset(v);
                }
                if let Some(v) = sort_by {
                    req = req.sort_by(v);
                }
                if let Some(v) = sort_direction {
                    req = req.sort_direction(v);
                }
                if let Some(v) = title {
                    req = req.title(v);
                }
                let result = req.send().await.map_err(data_err)?;
                Ok(result.into_iter().map(PyPosition::from).collect::<Vec<_>>())
            })
        })
    }

    #[pyo3(signature = (*, market=None))]
    fn positions_value(
        &self,
        py: Python<'_>,
        market: Option<Vec<String>>,
    ) -> PyResult<Vec<PyUserValue>> {
        let client = self.client.clone();
        let address = self.address.clone();
        py.detach(|| {
            crate::runtime::runtime().block_on(async move {
                let mut req = client.user(&address).positions_value();
                if let Some(v) = market {
                    req = req.market(v);
                }
                let result = req.send().await.map_err(data_err)?;
                Ok(result
                    .into_iter()
                    .map(PyUserValue::from)
                    .collect::<Vec<_>>())
            })
        })
    }

    #[pyo3(signature = (*, market=None, event_id=None, title=None, limit=None, offset=None, sort_by=None, sort_direction=None))]
    #[allow(clippy::too_many_arguments)]
    fn closed_positions(
        &self,
        py: Python<'_>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        title: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> PyResult<Vec<PyClosedPosition>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let sort_by = sort_by
            .map(|s| parse_closed_position_sort_by(&s))
            .transpose()?;
        let sort_direction = sort_direction
            .map(|s| parse_sort_direction(&s))
            .transpose()?;
        py.detach(|| {
            crate::runtime::runtime().block_on(async move {
                let mut req = client.user(&address).closed_positions();
                if let Some(v) = market {
                    req = req.market(v);
                }
                if let Some(v) = event_id {
                    req = req.event_id(v);
                }
                if let Some(v) = title {
                    req = req.title(v);
                }
                if let Some(v) = limit {
                    req = req.limit(v);
                }
                if let Some(v) = offset {
                    req = req.offset(v);
                }
                if let Some(v) = sort_by {
                    req = req.sort_by(v);
                }
                if let Some(v) = sort_direction {
                    req = req.sort_direction(v);
                }
                let result = req.send().await.map_err(data_err)?;
                Ok(result
                    .into_iter()
                    .map(PyClosedPosition::from)
                    .collect::<Vec<_>>())
            })
        })
    }

    #[pyo3(signature = (*, market=None, event_id=None, side=None, taker_only=None, filter_type=None, filter_amount=None, limit=None, offset=None))]
    #[allow(clippy::too_many_arguments)]
    fn trades(
        &self,
        py: Python<'_>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        side: Option<String>,
        taker_only: Option<bool>,
        filter_type: Option<String>,
        filter_amount: Option<f64>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<PyTrade>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let side = side.map(|s| parse_trade_side(&s)).transpose()?;
        let filter_type = filter_type
            .map(|s| parse_trade_filter_type(&s))
            .transpose()?;
        py.detach(|| {
            crate::runtime::runtime().block_on(async move {
                let mut req = client.user(&address).trades();
                if let Some(v) = market {
                    req = req.market(v);
                }
                if let Some(v) = event_id {
                    req = req.event_id(v);
                }
                if let Some(v) = side {
                    req = req.side(v);
                }
                if let Some(v) = taker_only {
                    req = req.taker_only(v);
                }
                if let Some(v) = filter_type {
                    req = req.filter_type(v);
                }
                if let Some(v) = filter_amount {
                    req = req.filter_amount(v);
                }
                if let Some(v) = limit {
                    req = req.limit(v);
                }
                if let Some(v) = offset {
                    req = req.offset(v);
                }
                let result = req.send().await.map_err(data_err)?;
                Ok(result.into_iter().map(PyTrade::from).collect::<Vec<_>>())
            })
        })
    }

    #[pyo3(signature = (*, market=None, event_id=None, activity_type=None, side=None, start=None, end=None, limit=None, offset=None, sort_by=None, sort_direction=None))]
    #[allow(clippy::too_many_arguments)]
    fn activity(
        &self,
        py: Python<'_>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        activity_type: Option<Vec<String>>,
        side: Option<String>,
        start: Option<i64>,
        end: Option<i64>,
        limit: Option<u32>,
        offset: Option<u32>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> PyResult<Vec<PyActivity>> {
        let client = self.client.clone();
        let address = self.address.clone();
        let activity_type = activity_type
            .map(|v| {
                v.iter()
                    .map(|s| parse_activity_type(s))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let side = side.map(|s| parse_trade_side(&s)).transpose()?;
        let sort_by = sort_by.map(|s| parse_activity_sort_by(&s)).transpose()?;
        let sort_direction = sort_direction
            .map(|s| parse_sort_direction(&s))
            .transpose()?;
        py.detach(|| {
            crate::runtime::runtime().block_on(async move {
                let mut req = client.user(&address).activity();
                if let Some(v) = market {
                    req = req.market(v);
                }
                if let Some(v) = event_id {
                    req = req.event_id(v);
                }
                if let Some(v) = activity_type {
                    req = req.activity_type(v);
                }
                if let Some(v) = side {
                    req = req.side(v);
                }
                if let Some(v) = start {
                    req = req.start(v);
                }
                if let Some(v) = end {
                    req = req.end(v);
                }
                if let Some(v) = limit {
                    req = req.limit(v);
                }
                if let Some(v) = offset {
                    req = req.offset(v);
                }
                if let Some(v) = sort_by {
                    req = req.sort_by(v);
                }
                if let Some(v) = sort_direction {
                    req = req.sort_direction(v);
                }
                let result = req.send().await.map_err(data_err)?;
                Ok(result.into_iter().map(PyActivity::from).collect::<Vec<_>>())
            })
        })
    }

    #[pyo3(signature = ())]
    fn traded(&self, py: Python<'_>) -> PyResult<PyUserTraded> {
        let client = self.client.clone();
        let address = self.address.clone();
        py.detach(|| {
            crate::runtime::runtime().block_on(async move {
                Ok(PyUserTraded::from(
                    client.user(&address).traded().await.map_err(data_err)?,
                ))
            })
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Trades
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiTrades,
    sync_name = PyDataApiTradesSync,
    py_async_name = "DataApiTrades",
    py_sync_name = "DataApiTradesSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = (*, user=None, market=None, event_id=None, side=None, taker_only=None, filter_type=None, filter_amount=None, limit=None, offset=None))]
    #[allow(clippy::too_many_arguments)]
    fn list(
        user: Option<String>,
        market: Option<Vec<String>>,
        event_id: Option<Vec<String>>,
        side: Option<String>,
        taker_only: Option<bool>,
        filter_type: Option<String>,
        filter_amount: Option<f64>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Vec<PyTrade> {
        let mut req = client.trades().list();
        if let Some(v) = user {
            req = req.user(v);
        }
        if let Some(v) = market {
            req = req.market(v);
        }
        if let Some(v) = event_id {
            req = req.event_id(v);
        }
        if let Some(v) = side {
            req = req.side(parse_trade_side(&v)?);
        }
        if let Some(v) = taker_only {
            req = req.taker_only(v);
        }
        if let Some(v) = filter_type {
            req = req.filter_type(parse_trade_filter_type(&v)?);
        }
        if let Some(v) = filter_amount {
            req = req.filter_amount(v);
        }
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        let result = req.send().await.map_err(data_err)?;
        Ok(result.into_iter().map(PyTrade::from).collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Holders
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiHolders,
    sync_name = PyDataApiHoldersSync,
    py_async_name = "DataApiHolders",
    py_sync_name = "DataApiHoldersSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = (markets, *, limit=None, min_balance=None))]
    fn list(
        markets: Vec<String>,
        limit: Option<u32>,
        min_balance: Option<u32>,
    ) -> Vec<PyMarketHolders> {
        let mut req = client.holders().list(markets);
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = min_balance {
            req = req.min_balance(v);
        }
        let result = req.send().await.map_err(data_err)?;
        Ok(result
            .into_iter()
            .map(PyMarketHolders::from)
            .collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Open Interest
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiOpenInterest,
    sync_name = PyDataApiOpenInterestSync,
    py_async_name = "DataApiOpenInterest",
    py_sync_name = "DataApiOpenInterestSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = (*, market=None))]
    fn get(market: Option<Vec<String>>) -> Vec<PyOpenInterest> {
        let mut req = client.open_interest().get();
        if let Some(v) = market {
            req = req.market(v);
        }
        let result = req.send().await.map_err(data_err)?;
        Ok(result
            .into_iter()
            .map(PyOpenInterest::from)
            .collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Live Volume
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiLiveVolume,
    sync_name = PyDataApiLiveVolumeSync,
    py_async_name = "DataApiLiveVolume",
    py_sync_name = "DataApiLiveVolumeSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = (event_id,))]
    fn get(event_id: u64) -> Vec<PyLiveVolume> {
        let result = client.live_volume().get(event_id).await.map_err(data_err)?;
        Ok(result
            .into_iter()
            .map(PyLiveVolume::from)
            .collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Leaderboard
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiLeaderboard,
    sync_name = PyDataApiLeaderboardSync,
    py_async_name = "DataApiLeaderboard",
    py_sync_name = "DataApiLeaderboardSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = (*, category=None, time_period=None, order_by=None, limit=None, offset=None, user=None, user_name=None))]
    fn get(
        category: Option<String>,
        time_period: Option<String>,
        order_by: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
        user: Option<String>,
        user_name: Option<String>,
    ) -> Vec<PyTraderRanking> {
        let mut req = client.leaderboard().get();
        if let Some(v) = category {
            req = req.category(parse_leaderboard_category(&v)?);
        }
        if let Some(v) = time_period {
            req = req.time_period(parse_time_period(&v)?);
        }
        if let Some(v) = order_by {
            req = req.order_by(parse_leaderboard_order_by(&v)?);
        }
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = user {
            req = req.user(v);
        }
        if let Some(v) = user_name {
            req = req.user_name(v);
        }
        let result = req.send().await.map_err(data_err)?;
        Ok(result
            .into_iter()
            .map(PyTraderRanking::from)
            .collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Builders
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiBuilders,
    sync_name = PyDataApiBuildersSync,
    py_async_name = "DataApiBuilders",
    py_sync_name = "DataApiBuildersSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = (*, time_period=None, limit=None, offset=None))]
    fn leaderboard(
        time_period: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Vec<PyBuilderRanking> {
        let mut req = client.builders().leaderboard();
        if let Some(v) = time_period {
            req = req.time_period(parse_time_period(&v)?);
        }
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        let result = req.send().await.map_err(data_err)?;
        Ok(result
            .into_iter()
            .map(PyBuilderRanking::from)
            .collect::<Vec<_>>())
    },
    #[pyo3(signature = (*, time_period=None))]
    fn volume(time_period: Option<String>) -> Vec<PyBuilderVolume> {
        let mut req = client.builders().volume();
        if let Some(v) = time_period {
            req = req.time_period(parse_time_period(&v)?);
        }
        let result = req.send().await.map_err(data_err)?;
        Ok(result
            .into_iter()
            .map(PyBuilderVolume::from)
            .collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Health
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyDataApiHealth,
    sync_name = PyDataApiHealthSync,
    py_async_name = "DataApiHealth",
    py_sync_name = "DataApiHealthSync",
    client_type = polyoxide_data::DataApi,
    client_var = client,
    #[pyo3(signature = ())]
    fn ping() -> f64 {
        let duration = client.health().ping().await.map_err(data_err)?;
        Ok(duration.as_secs_f64())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Async Client
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "DataApi", skip_from_py_object)]
pub struct PyDataApi {
    client: Arc<polyoxide_data::DataApi>,
}

#[pymethods]
impl PyDataApi {
    #[new]
    #[pyo3(signature = (*, base_url=None, timeout_ms=None, pool_size=None))]
    fn new(
        base_url: Option<String>,
        timeout_ms: Option<u64>,
        pool_size: Option<usize>,
    ) -> PyResult<Self> {
        let mut builder = polyoxide_data::DataApi::builder();
        if let Some(v) = base_url {
            builder = builder.base_url(v);
        }
        if let Some(v) = timeout_ms {
            builder = builder.timeout_ms(v);
        }
        if let Some(v) = pool_size {
            builder = builder.pool_size(v);
        }
        let client = builder.build().map_err(data_err)?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    fn user(&self, address: String) -> PyDataApiUser {
        PyDataApiUser {
            client: self.client.clone(),
            address,
        }
    }

    fn trades(&self) -> PyDataApiTrades {
        PyDataApiTrades {
            client: self.client.clone(),
        }
    }

    fn holders(&self) -> PyDataApiHolders {
        PyDataApiHolders {
            client: self.client.clone(),
        }
    }

    fn open_interest(&self) -> PyDataApiOpenInterest {
        PyDataApiOpenInterest {
            client: self.client.clone(),
        }
    }

    fn live_volume(&self) -> PyDataApiLiveVolume {
        PyDataApiLiveVolume {
            client: self.client.clone(),
        }
    }

    fn leaderboard(&self) -> PyDataApiLeaderboard {
        PyDataApiLeaderboard {
            client: self.client.clone(),
        }
    }

    fn builders(&self) -> PyDataApiBuilders {
        PyDataApiBuilders {
            client: self.client.clone(),
        }
    }

    fn health(&self) -> PyDataApiHealth {
        PyDataApiHealth {
            client: self.client.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sync Client
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "DataApiSync", skip_from_py_object)]
pub struct PyDataApiSync {
    client: Arc<polyoxide_data::DataApi>,
}

#[pymethods]
impl PyDataApiSync {
    #[new]
    #[pyo3(signature = (*, base_url=None, timeout_ms=None, pool_size=None))]
    fn new(
        base_url: Option<String>,
        timeout_ms: Option<u64>,
        pool_size: Option<usize>,
    ) -> PyResult<Self> {
        let mut builder = polyoxide_data::DataApi::builder();
        if let Some(v) = base_url {
            builder = builder.base_url(v);
        }
        if let Some(v) = timeout_ms {
            builder = builder.timeout_ms(v);
        }
        if let Some(v) = pool_size {
            builder = builder.pool_size(v);
        }
        let client = builder.build().map_err(data_err)?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    fn user(&self, address: String) -> PyDataApiUserSync {
        PyDataApiUserSync {
            client: self.client.clone(),
            address,
        }
    }

    fn trades(&self) -> PyDataApiTradesSync {
        PyDataApiTradesSync {
            client: self.client.clone(),
        }
    }

    fn holders(&self) -> PyDataApiHoldersSync {
        PyDataApiHoldersSync {
            client: self.client.clone(),
        }
    }

    fn open_interest(&self) -> PyDataApiOpenInterestSync {
        PyDataApiOpenInterestSync {
            client: self.client.clone(),
        }
    }

    fn live_volume(&self) -> PyDataApiLiveVolumeSync {
        PyDataApiLiveVolumeSync {
            client: self.client.clone(),
        }
    }

    fn leaderboard(&self) -> PyDataApiLeaderboardSync {
        PyDataApiLeaderboardSync {
            client: self.client.clone(),
        }
    }

    fn builders(&self) -> PyDataApiBuildersSync {
        PyDataApiBuildersSync {
            client: self.client.clone(),
        }
    }

    fn health(&self) -> PyDataApiHealthSync {
        PyDataApiHealthSync {
            client: self.client.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Registration
// ═══════════════════════════════════════════════════════════════════════════════

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyDataApi>()?;
    m.add_class::<PyDataApiSync>()?;
    m.add_class::<PyDataApiUser>()?;
    m.add_class::<PyDataApiUserSync>()?;
    m.add_class::<PyDataApiTrades>()?;
    m.add_class::<PyDataApiTradesSync>()?;
    m.add_class::<PyDataApiHolders>()?;
    m.add_class::<PyDataApiHoldersSync>()?;
    m.add_class::<PyDataApiOpenInterest>()?;
    m.add_class::<PyDataApiOpenInterestSync>()?;
    m.add_class::<PyDataApiLiveVolume>()?;
    m.add_class::<PyDataApiLiveVolumeSync>()?;
    m.add_class::<PyDataApiLeaderboard>()?;
    m.add_class::<PyDataApiLeaderboardSync>()?;
    m.add_class::<PyDataApiBuilders>()?;
    m.add_class::<PyDataApiBuildersSync>()?;
    m.add_class::<PyDataApiHealth>()?;
    m.add_class::<PyDataApiHealthSync>()?;
    Ok(())
}
