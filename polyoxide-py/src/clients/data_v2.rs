//! Data API v2: `DataApi.v2()` / `DataApiSync.v2()`, registered on the
//! `polyoxide.v2` submodule with the row classes in `types::data_v2`.
//!
//! Every route is declared once, in the `v2_namespaces!` invocation at the end
//! of this file, which emits both the async and the sync class. Arguments are
//! parsed before any request is made, so a bad value raises `ValueError` at
//! the call rather than when the coroutine is awaited.

use std::{convert::Infallible, fmt::Display, pin::Pin, str::FromStr, sync::Arc};

use futures_util::{lock::Mutex, Stream, StreamExt};
use polyoxide_data::{
    types::SortDirection,
    v2::{
        self,
        types::{PositionAnchor, ResolutionKey},
        DataV2,
    },
    DataApiError,
};
use pyo3::{
    exceptions::{PyStopAsyncIteration, PyValueError},
    prelude::*,
    types::PyModuleMethods,
    PyClass, PyClassInitializer,
};

use crate::{error::data_err, runtime::runtime, types::data_v2::*};

/// Parses a request-only enum from its wire spelling, listing every accepted
/// value when it is not one of them.
fn choice<T: FromStr + Display>(param: &str, value: &str, all: &[T]) -> PyResult<T> {
    value.parse().map_err(|_| {
        let accepted = all
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        PyValueError::new_err(format!(
            "invalid {param} {value:?}, expected one of: {accepted}"
        ))
    })
}

/// Parses an enum that also appears in responses. A value this SDK does not
/// know is sent verbatim, so a value upstream adds later still works.
fn open<T: FromStr<Err = Infallible>>(value: &str) -> T {
    let Ok(parsed) = value.parse();
    parsed
}

/// `sort_direction`, spelled exactly as on the wire like every other enum here.
fn direction(value: &str) -> PyResult<SortDirection> {
    match value {
        "ASC" => Ok(SortDirection::Asc),
        "DESC" => Ok(SortDirection::Desc),
        _ => Err(PyValueError::new_err(format!(
            "invalid sort_direction {value:?}, expected one of: ASC, DESC"
        ))),
    }
}

/// `positions` takes a wallet, one market, or a wallet narrowed to markets.
fn position_anchor(
    user: Option<String>,
    conditions: Option<Vec<String>>,
) -> PyResult<PositionAnchor> {
    match (user, conditions) {
        (Some(user), None) => Ok(PositionAnchor::User(user)),
        (Some(user), Some(conditions)) => Ok(PositionAnchor::UserInConditions { user, conditions }),
        (None, Some(mut conditions)) if conditions.len() == 1 => {
            Ok(PositionAnchor::Condition(conditions.remove(0)))
        }
        (None, Some(conditions)) => Err(PyValueError::new_err(format!(
            "positions without user takes exactly one condition, got {}",
            conditions.len()
        ))),
        (None, None) => Err(PyValueError::new_err(
            "positions needs user, conditions, or both",
        )),
    }
}

/// `resolutions` takes exactly one selector family.
fn resolution_key(
    question_id: Option<String>,
    conditions: Option<Vec<String>>,
    event_ids: Option<Vec<String>>,
) -> PyResult<ResolutionKey> {
    match (question_id, conditions, event_ids) {
        (Some(id), None, None) => Ok(ResolutionKey::Question(id)),
        (None, Some(ids), None) => Ok(ResolutionKey::Conditions(ids)),
        (None, None, Some(ids)) => Ok(ResolutionKey::Events(ids)),
        _ => Err(PyValueError::new_err(
            "resolutions takes exactly one of question_id, conditions or event_ids",
        )),
    }
}

/// One page of a paged route: its rows, already wrapped, and its pagination.
#[pyclass(name = "Page", skip_from_py_object)]
pub struct PyPage {
    data: Vec<Py<PyAny>>,
    pagination: PyV2Pagination,
}

impl PyPage {
    /// Wraps each row as `W`. The rows become Python objects here, once, rather
    /// than on every `data` access.
    fn new<T, W>(py: Python<'_>, page: v2::Page<T>) -> PyResult<Self>
    where
        W: From<T> + PyClass + Into<PyClassInitializer<W>>,
    {
        let data = page
            .data
            .into_iter()
            .map(|row| Py::new(py, W::from(row)).map(Py::into_any))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            data,
            pagination: PyV2Pagination::from(page.pagination),
        })
    }
}

#[pymethods]
impl PyPage {
    /// The page's rows.
    #[getter]
    fn data(&self, py: Python<'_>) -> Vec<Py<PyAny>> {
        self.data.iter().map(|row| row.clone_ref(py)).collect()
    }

    /// Paging state; follow `next_cursor` until it is `None`.
    #[getter]
    fn pagination(&self) -> PyV2Pagination {
        self.pagination.clone()
    }

    fn __len__(&self) -> usize {
        self.data.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "Page(rows={}, pagination={})",
            self.data.len(),
            self.pagination.inner()
        )
    }
}

/// Builds a `Page` once the GIL is held. Boxing it erases the row type, so one
/// iterator class serves every paged route.
type PageBuilder = Box<dyn for<'py> FnOnce(Python<'py>) -> PyResult<PyPage> + Send>;
type Pages = Arc<Mutex<Pin<Box<dyn Stream<Item = Result<PageBuilder, DataApiError>> + Send>>>>;

fn erase<T, W>(pages: v2::PageStream<T>) -> Pages
where
    T: Send + 'static,
    W: From<T> + PyClass + Into<PyClassInitializer<W>>,
{
    let pages = pages.map(|page| {
        page.map(|page| {
            Box::new(move |py: Python<'_>| PyPage::new::<T, W>(py, page)) as PageBuilder
        })
    });
    Arc::new(Mutex::new(Box::pin(pages)))
}

/// Async iterator over every page of a route, from `DataV2.iter_*`. Each page
/// re-sends the filters the walk started with.
#[pyclass(name = "PageIterator", skip_from_py_object)]
pub struct PyPageIterator {
    pages: Pages,
}

#[pymethods]
impl PyPageIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pages = self.pages.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let next = pages.lock().await.next().await;
            match next {
                Some(Ok(build)) => Python::attach(build),
                Some(Err(e)) => Err(data_err(e)),
                None => Err(PyStopAsyncIteration::new_err(())),
            }
        })
    }
}

/// Iterator over every page of a route, from `DataV2Sync.iter_*`.
#[pyclass(name = "PageIteratorSync", skip_from_py_object)]
pub struct PyPageIteratorSync {
    pages: Pages,
}

#[pymethods]
impl PyPageIteratorSync {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<PyPage>> {
        let pages = self.pages.clone();
        let next = py.detach(|| runtime().block_on(async move { pages.lock().await.next().await }));
        match next {
            Some(Ok(build)) => build(py).map(Some),
            Some(Err(e)) => Err(data_err(e)),
            None => Ok(None),
        }
    }
}

/// Emits `DataV2` and `DataV2Sync` from one list of routes.
///
/// Each entry names its parameters, the row class it returns, and an
/// expression that builds the request from `v2` (a `DataV2`). `plain` routes
/// return the row, `optional` routes the row or `None`, `list` routes a list,
/// and `paged` routes a `Page` plus an `iter_*` method that walks every page.
macro_rules! v2_namespaces {
    (
        plain { $(
            #[doc = $p_doc:literal]
            #[pyo3(signature = $p_sig:tt)]
            fn $p_name:ident($($p_arg:ident: $p_ty:ty),* $(,)?) -> $p_row:ident
                => |$p_v2:ident| $p_build:expr;
        )* }
        optional { $(
            #[doc = $o_doc:literal]
            #[pyo3(signature = $o_sig:tt)]
            fn $o_name:ident($($o_arg:ident: $o_ty:ty),* $(,)?) -> $o_row:ident
                => |$o_v2:ident| $o_build:expr;
        )* }
        list { $(
            #[doc = $l_doc:literal]
            #[pyo3(signature = $l_sig:tt)]
            fn $l_name:ident($($l_arg:ident: $l_ty:ty),* $(,)?) -> $l_row:ident
                => |$l_v2:ident| $l_build:expr;
        )* }
        paged { $(
            #[doc = $g_doc:literal]
            #[pyo3(signature = $g_sig:tt)]
            fn $g_name:ident / $g_iter:ident($($g_arg:ident: $g_ty:ty),* $(,)?) -> $g_row:ident
                => |$g_v2:ident| $g_build:expr;
        )* }
    ) => {
        /// Async Data API v2 routes. Obtain with `DataApi().v2()`.
        #[pyclass(name = "DataV2", skip_from_py_object)]
        pub struct PyDataV2 {
            pub(crate) v2: DataV2,
        }

        #[pymethods]
        impl PyDataV2 {
            $(
                #[doc = $p_doc]
                #[pyo3(signature = $p_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $p_name<'py>(&self, py: Python<'py>, $($p_arg: $p_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $p_v2 = &self.v2;
                    let request = $p_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        Ok($p_row::from(request.send().await.map_err(data_err)?))
                    })
                }
            )*
            $(
                #[doc = $o_doc]
                #[pyo3(signature = $o_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $o_name<'py>(&self, py: Python<'py>, $($o_arg: $o_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $o_v2 = &self.v2;
                    let request = $o_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        Ok(request.send().await.map_err(data_err)?.map($o_row::from))
                    })
                }
            )*
            $(
                #[doc = $l_doc]
                #[pyo3(signature = $l_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $l_name<'py>(&self, py: Python<'py>, $($l_arg: $l_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $l_v2 = &self.v2;
                    let request = $l_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        let rows = request.send().await.map_err(data_err)?;
                        Ok(rows.into_iter().map($l_row::from).collect::<Vec<_>>())
                    })
                }
            )*
            $(
                #[doc = $g_doc]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_name<'py>(&self, py: Python<'py>, $($g_arg: $g_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        let page = request.send().await.map_err(data_err)?;
                        Python::attach(|py| PyPage::new::<_, $g_row>(py, page))
                    })
                }

                #[doc = concat!("Walks every page of `", stringify!($g_name), "`, re-sending these filters on each one.")]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_iter(&self, $($g_arg: $g_ty),*) -> PyResult<PyPageIterator> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    Ok(PyPageIterator { pages: erase::<_, $g_row>(request.pages()) })
                }
            )*
        }

        /// Sync Data API v2 routes. Obtain with `DataApiSync().v2()`.
        #[pyclass(name = "DataV2Sync", skip_from_py_object)]
        pub struct PyDataV2Sync {
            pub(crate) v2: DataV2,
        }

        #[pymethods]
        impl PyDataV2Sync {
            $(
                #[doc = $p_doc]
                #[pyo3(signature = $p_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $p_name(&self, py: Python<'_>, $($p_arg: $p_ty),*) -> PyResult<$p_row> {
                    let $p_v2 = &self.v2;
                    let request = $p_build;
                    let row = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    Ok($p_row::from(row))
                }
            )*
            $(
                #[doc = $o_doc]
                #[pyo3(signature = $o_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $o_name(&self, py: Python<'_>, $($o_arg: $o_ty),*) -> PyResult<Option<$o_row>> {
                    let $o_v2 = &self.v2;
                    let request = $o_build;
                    let row = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    Ok(row.map($o_row::from))
                }
            )*
            $(
                #[doc = $l_doc]
                #[pyo3(signature = $l_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $l_name(&self, py: Python<'_>, $($l_arg: $l_ty),*) -> PyResult<Vec<$l_row>> {
                    let $l_v2 = &self.v2;
                    let request = $l_build;
                    let rows = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    Ok(rows.into_iter().map($l_row::from).collect())
                }
            )*
            $(
                #[doc = $g_doc]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_name(&self, py: Python<'_>, $($g_arg: $g_ty),*) -> PyResult<PyPage> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    let page = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    PyPage::new::<_, $g_row>(py, page)
                }

                #[doc = concat!("Walks every page of `", stringify!($g_name), "`, re-sending these filters on each one.")]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_iter(&self, $($g_arg: $g_ty),*) -> PyResult<PyPageIteratorSync> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    Ok(PyPageIteratorSync { pages: erase::<_, $g_row>(request.pages()) })
                }
            )*
        }
    };
}

v2_namespaces! {
    plain {
        /// `GET /v2/approvals`: a wallet's Polygon token and operator approvals.
        #[pyo3(signature = (user))]
        fn approvals(
            user: String,
        ) -> PyV2Approvals => |client| client.approvals(user);

        /// `GET /v2/user-pnl`: a wallet's cumulative PnL series. Not the same series as `data.pnl()`.
        #[pyo3(signature = (user, *, interval=None, fidelity=None))]
        fn user_pnl(
            user: String,
            interval: Option<String>,
            fidelity: Option<String>,
        ) -> PyV2UserPnlSeries => |client| {
            let mut request = client.user_pnl(user);
            if let Some(v) = interval {
                request = request.interval(choice("interval", &v, v2::types::PnlInterval::ALL)?);
            }
            if let Some(v) = fidelity {
                request = request.fidelity(choice("fidelity", &v, v2::types::PnlFidelity::ALL)?);
            }
            request
        };

        /// `GET /v2/user-volume`: a wallet's traded volume over a whole-day window.
        #[pyo3(signature = (user, *, start=None, end=None))]
        fn user_volume(
            user: String,
            start: Option<i64>,
            end: Option<i64>,
        ) -> PyV2UserVolume => |client| {
            let mut request = client.user_volume(user);
            if let Some(v) = start {
                request = request.start(v);
            }
            if let Some(v) = end {
                request = request.end(v);
            }
            request
        };

        /// `GET /v2/value`: a wallet's portfolio value in USDC.
        #[pyo3(signature = (user, *, conditions=None))]
        fn value(
            user: String,
            conditions: Option<Vec<String>>,
        ) -> PyV2PortfolioValue => |client| {
            let mut request = client.value(user);
            if let Some(v) = conditions {
                request = request.conditions(v);
            }
            request
        };

        /// `GET /v2/live-volume`: taker volume per market under these Gamma events.
        #[pyo3(signature = (event_ids))]
        fn live_volume(
            event_ids: Vec<String>,
        ) -> PyV2LiveVolume => |client| client.live_volume(event_ids);

        /// `GET /v2/status`: how fresh the served data is. This is not a liveness check.
        #[pyo3(signature = ())]
        fn status() -> PyV2ServiceStatus => |client| client.status();
    }

    optional {
        /// `GET /v2/user-stats`: a wallet's profile card, or None for a wallet the API does not know.
        #[pyo3(signature = (user))]
        fn user_stats(
            user: String,
        ) -> PyV2UserStats => |client| client.user_stats(user);

        /// `GET /v2/leaderboard`: one wallet's standing on both boards, or None for a wallet the API does not know.
        #[pyo3(signature = (user, *, time_period=None, category=None))]
        fn leaderboard_user(
            user: String,
            time_period: Option<String>,
            category: Option<String>,
        ) -> PyV2LeaderboardUserEntry => |client| {
            let mut request = client.leaderboard_user(user);
            if let Some(v) = time_period {
                request = request.time_period(choice("time_period", &v, v2::types::TimePeriod::ALL)?);
            }
            if let Some(v) = category {
                request = request.category(v);
            }
            request
        };
    }

    list {
        /// `GET /v2/oi`: open interest per market; one GLOBAL row without `conditions`.
        #[pyo3(signature = (*, conditions=None))]
        fn open_interest(
            conditions: Option<Vec<String>>,
        ) -> PyV2OpenInterest => |client| {
            let mut request = client.open_interest();
            if let Some(v) = conditions {
                request = request.conditions(v);
            }
            request
        };

        /// `GET /v2/resolutions`: resolution state, by exactly one of `question_id`, `conditions` or `event_ids`.
        #[pyo3(signature = (*, question_id=None, conditions=None, event_ids=None))]
        fn resolutions(
            question_id: Option<String>,
            conditions: Option<Vec<String>>,
            event_ids: Option<Vec<String>>,
        ) -> PyV2Resolution => |client| client.resolutions(resolution_key(question_id, conditions, event_ids)?);

        /// `GET /v2/builders/volume`: builder volume per time bucket.
        #[pyo3(signature = (*, interval=None, limit=None))]
        fn builder_volume(
            interval: Option<String>,
            limit: Option<u32>,
        ) -> PyV2BuilderVolumePoint => |client| {
            let mut request = client.builder_volume();
            if let Some(v) = interval {
                request = request.interval(choice("interval", &v, v2::types::TimePeriod::ALL)?);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            request
        };
    }

    paged {
        /// `GET /v2/positions`: positions for a wallet (`user`), one market (`conditions` of one id), or a wallet in some markets (both).
        #[pyo3(signature = (*, user=None, conditions=None, status=None, event_ids=None, title=None, filter_type=None, filter_amount=None, include_archived=None, sort_by=None, sort_direction=None, start=None, end=None, limit=None, cursor=None))]
        fn positions / iter_positions(
            user: Option<String>,
            conditions: Option<Vec<String>>,
            status: Option<String>,
            event_ids: Option<Vec<String>>,
            title: Option<String>,
            filter_type: Option<String>,
            filter_amount: Option<f64>,
            include_archived: Option<bool>,
            sort_by: Option<String>,
            sort_direction: Option<String>,
            start: Option<i64>,
            end: Option<i64>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2Position => |client| {
            let mut request = client.positions(position_anchor(user, conditions)?);
            if let Some(v) = status {
                request = request.status(open(&v));
            }
            if let Some(v) = event_ids {
                request = request.event_ids(v);
            }
            if let Some(v) = title {
                request = request.title(v);
            }
            if let Some(v) = filter_type {
                request = request.filter_type(choice("filter_type", &v, v2::types::FilterType::ALL)?);
            }
            if let Some(v) = filter_amount {
                request = request.filter_amount(v);
            }
            if let Some(v) = include_archived {
                request = request.include_archived(v);
            }
            if let Some(v) = sort_by {
                request = request.sort_by(choice("sort_by", &v, v2::types::PositionSortBy::ALL)?);
            }
            if let Some(v) = sort_direction {
                request = request.sort_direction(direction(&v)?);
            }
            if let Some(v) = start {
                request = request.start(v);
            }
            if let Some(v) = end {
                request = request.end(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/positions/combos`: a wallet's combo positions.
        #[pyo3(signature = (user, *, conditions=None, statuses=None, sort_by=None, sort_direction=None, updated_after=None, updated_before=None, limit=None, cursor=None))]
        fn combo_positions / iter_combo_positions(
            user: String,
            conditions: Option<Vec<String>>,
            statuses: Option<Vec<String>>,
            sort_by: Option<String>,
            sort_direction: Option<String>,
            updated_after: Option<i64>,
            updated_before: Option<i64>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2ComboPosition => |client| {
            let mut request = client.combo_positions(user);
            if let Some(v) = conditions {
                request = request.conditions(v);
            }
            if let Some(v) = statuses {
                request = request.statuses(v.iter().map(|s| choice("statuses", s, v2::types::ComboPositionStatus::ALL)).collect::<PyResult<Vec<_>>>()?);
            }
            if let Some(v) = sort_by {
                request = request.sort_by(choice("sort_by", &v, v2::types::ComboPositionSortBy::ALL)?);
            }
            if let Some(v) = sort_direction {
                request = request.sort_direction(direction(&v)?);
            }
            if let Some(v) = updated_after {
                request = request.updated_after(v);
            }
            if let Some(v) = updated_before {
                request = request.updated_before(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/activity`: a wallet's activity feed.
        #[pyo3(signature = (user, *, types=None, conditions=None, event_ids=None, side=None, start=None, end=None, sort_by=None, sort_direction=None, exclude_deposits_withdrawals=None, limit=None, cursor=None))]
        fn activity / iter_activity(
            user: String,
            types: Option<Vec<String>>,
            conditions: Option<Vec<String>>,
            event_ids: Option<Vec<String>>,
            side: Option<String>,
            start: Option<i64>,
            end: Option<i64>,
            sort_by: Option<String>,
            sort_direction: Option<String>,
            exclude_deposits_withdrawals: Option<bool>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2Activity => |client| {
            let mut request = client.activity(user);
            if let Some(v) = types {
                request = request.types(v.iter().map(|s| open::<v2::types::ActivityType>(s)).collect::<Vec<_>>());
            }
            if let Some(v) = conditions {
                request = request.conditions(v);
            }
            if let Some(v) = event_ids {
                request = request.event_ids(v);
            }
            if let Some(v) = side {
                request = request.side(open(&v));
            }
            if let Some(v) = start {
                request = request.start(v);
            }
            if let Some(v) = end {
                request = request.end(v);
            }
            if let Some(v) = sort_by {
                request = request.sort_by(choice("sort_by", &v, v2::types::ActivitySortBy::ALL)?);
            }
            if let Some(v) = sort_direction {
                request = request.sort_direction(direction(&v)?);
            }
            if let Some(v) = exclude_deposits_withdrawals {
                request = request.exclude_deposits_withdrawals(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/activity/combos`: a wallet's combo lifecycle events.
        #[pyo3(signature = (user, *, conditions=None, limit=None, cursor=None))]
        fn combo_activity / iter_combo_activity(
            user: String,
            conditions: Option<Vec<String>>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2ComboActivity => |client| {
            let mut request = client.combo_activity(user);
            if let Some(v) = conditions {
                request = request.conditions(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/trades`: the trade feed, newest first.
        #[pyo3(signature = (*, user=None, conditions=None, event_ids=None, side=None, taker_only=None, filter_type=None, filter_amount=None, start=None, end=None, limit=None, cursor=None))]
        fn trades / iter_trades(
            user: Option<String>,
            conditions: Option<Vec<String>>,
            event_ids: Option<Vec<String>>,
            side: Option<String>,
            taker_only: Option<bool>,
            filter_type: Option<String>,
            filter_amount: Option<f64>,
            start: Option<i64>,
            end: Option<i64>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2Trade => |client| {
            let mut request = client.trades();
            if let Some(v) = user {
                request = request.user(v);
            }
            if let Some(v) = conditions {
                request = request.conditions(v);
            }
            if let Some(v) = event_ids {
                request = request.event_ids(v);
            }
            if let Some(v) = side {
                request = request.side(open(&v));
            }
            if let Some(v) = taker_only {
                request = request.taker_only(v);
            }
            if let Some(v) = filter_type {
                request = request.filter_type(choice("filter_type", &v, v2::types::FilterType::ALL)?);
            }
            if let Some(v) = filter_amount {
                request = request.filter_amount(v);
            }
            if let Some(v) = start {
                request = request.start(v);
            }
            if let Some(v) = end {
                request = request.end(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/holders`: top holders per outcome token, for these markets (at most 20).
        #[pyo3(signature = (conditions, *, min_balance=None, include_pnl=None, limit=None, cursor=None))]
        fn holders / iter_holders(
            conditions: Vec<String>,
            min_balance: Option<f64>,
            include_pnl: Option<bool>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2MetaHolder => |client| {
            let mut request = client.holders(conditions);
            if let Some(v) = min_balance {
                request = request.min_balance(v);
            }
            if let Some(v) = include_pnl {
                request = request.include_pnl(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/prices-history`: an outcome token's price series.
        #[pyo3(signature = (token_id, *, start=None, end=None, interval=None, bucket_seconds=None, as_of=None, limit=None, cursor=None))]
        fn prices_history / iter_prices_history(
            token_id: String,
            start: Option<i64>,
            end: Option<i64>,
            interval: Option<String>,
            bucket_seconds: Option<u32>,
            as_of: Option<i64>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2PricePoint => |client| {
            let mut request = client.prices_history(token_id);
            if let Some(v) = start {
                request = request.start(v);
            }
            if let Some(v) = end {
                request = request.end(v);
            }
            if let Some(v) = interval {
                request = request.interval(choice("interval", &v, v2::types::PricesInterval::ALL)?);
            }
            if let Some(v) = bucket_seconds {
                request = request.bucket_seconds(v);
            }
            if let Some(v) = as_of {
                request = request.as_of(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/biggest-winners`: the largest single winning positions in a window.
        #[pyo3(signature = (*, time_period=None, category=None, limit=None, cursor=None))]
        fn biggest_winners / iter_biggest_winners(
            time_period: Option<String>,
            category: Option<String>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2BiggestWinner => |client| {
            let mut request = client.biggest_winners();
            if let Some(v) = time_period {
                request = request.time_period(choice("time_period", &v, v2::types::TimePeriod::ALL)?);
            }
            if let Some(v) = category {
                request = request.category(v);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/builders/leaderboard`: builders ranked by volume for a window.
        #[pyo3(signature = (*, time_period=None, limit=None, cursor=None))]
        fn builders_leaderboard / iter_builders_leaderboard(
            time_period: Option<String>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2BuilderStanding => |client| {
            let mut request = client.builders_leaderboard();
            if let Some(v) = time_period {
                request = request.time_period(choice("time_period", &v, v2::types::TimePeriod::ALL)?);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };

        /// `GET /v2/leaderboard`: the trader board. `board` picks PNL or VOLUME; volume is in shares, not USDC.
        #[pyo3(signature = (*, time_period=None, category=None, board=None, limit=None, cursor=None))]
        fn leaderboard / iter_leaderboard(
            time_period: Option<String>,
            category: Option<String>,
            board: Option<String>,
            limit: Option<u32>,
            cursor: Option<String>,
        ) -> PyV2LeaderboardEntry => |client| {
            let mut request = client.leaderboard();
            if let Some(v) = time_period {
                request = request.time_period(choice("time_period", &v, v2::types::TimePeriod::ALL)?);
            }
            if let Some(v) = category {
                request = request.category(v);
            }
            if let Some(v) = board {
                request = request.board(choice("board", &v, v2::types::LeaderboardBoard::ALL)?);
            }
            if let Some(v) = limit {
                request = request.limit(v);
            }
            if let Some(v) = cursor {
                request = request.cursor(v);
            }
            request
        };
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDataV2>()?;
    m.add_class::<PyDataV2Sync>()?;
    m.add_class::<PyPage>()?;
    m.add_class::<PyPageIterator>()?;
    m.add_class::<PyPageIteratorSync>()?;
    Ok(())
}
