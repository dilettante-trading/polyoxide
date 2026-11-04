//! The Data API client. Polymarket retired Data API v1 on 2026-11-02, so its
//! routes are v2's, through `v2()`.

use pyo3::prelude::*;
use pyo3::types::PyModuleMethods;
use std::sync::Arc;

use crate::error::data_err;

#[pyclass(name = "DataApi", skip_from_py_object)]
pub struct PyDataApi {
    client: Arc<polyoxide_polymarket::data::DataApi>,
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
        let mut builder = polyoxide_polymarket::data::DataApi::builder();
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

    /// The Data API v2 routes, sharing this client's connection pool and rate limiter.
    fn v2(&self) -> super::data_v2::PyDataV2 {
        super::data_v2::PyDataV2 {
            v2: self.client.v2(),
        }
    }
}

#[pyclass(name = "DataApiSync", skip_from_py_object)]
pub struct PyDataApiSync {
    client: Arc<polyoxide_polymarket::data::DataApi>,
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
        let mut builder = polyoxide_polymarket::data::DataApi::builder();
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

    /// The Data API v2 routes, sharing this client's connection pool and rate limiter.
    fn v2(&self) -> super::data_v2::PyDataV2Sync {
        super::data_v2::PyDataV2Sync {
            v2: self.client.v2(),
        }
    }
}

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyDataApi>()?;
    m.add_class::<PyDataApiSync>()?;
    Ok(())
}
