use pyo3::prelude::*;

#[pymodule]
fn _polyoxide(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = m;
    Ok(())
}
