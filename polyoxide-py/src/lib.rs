use pyo3::prelude::*;

#[macro_use]
mod macros;
mod clients;
mod convert;
mod error;
mod runtime;
mod types;

#[pymodule]
fn _polyoxide(m: &Bound<'_, PyModule>) -> PyResult<()> {
    error::register(m)?;
    types::register(m)?;
    clients::register(m)?;

    // v2 row classes reuse v1 names (`Trade`, `Position`, ...), so they live
    // in their own submodule, re-exported as `polyoxide.v2`.
    let v2 = PyModule::new(m.py(), "v2")?;
    types::data_v2::register(&v2)?;
    clients::data_v2::register(&v2)?;
    m.add_submodule(&v2)?;
    Ok(())
}
