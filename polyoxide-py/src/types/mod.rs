mod clob;
mod data;
mod gamma;

pub use clob::*;
pub use data::*;
pub use gamma::*;

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    gamma::register(m)?;
    data::register(m)?;
    clob::register(m)?;
    Ok(())
}
