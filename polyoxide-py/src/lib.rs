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
    Ok(())
}
