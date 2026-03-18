use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(polyoxide, PolyoxideError, PyException);
create_exception!(polyoxide, ApiError, PolyoxideError);
create_exception!(polyoxide, AuthenticationError, PolyoxideError);
create_exception!(polyoxide, ValidationError, PolyoxideError);
create_exception!(polyoxide, RateLimitError, PolyoxideError);
create_exception!(polyoxide, NetworkError, PolyoxideError);
create_exception!(polyoxide, TimeoutError, PolyoxideError);

pub fn gamma_err(e: polyoxide_gamma::GammaError) -> PyErr {
    map_api_err(&e)
}

pub fn data_err(e: polyoxide_data::DataApiError) -> PyErr {
    map_api_err(&e)
}

pub fn clob_err(e: polyoxide_clob::ClobError) -> PyErr {
    map_api_err(&e)
}

fn map_api_err(e: &dyn std::fmt::Display) -> PyErr {
    let msg = e.to_string();
    if msg.contains("Authentication") {
        AuthenticationError::new_err(msg)
    } else if msg.contains("Rate limit") || msg.contains("429") {
        RateLimitError::new_err(msg)
    } else if msg.contains("Validation") {
        ValidationError::new_err(msg)
    } else if msg.contains("timeout") || msg.contains("Timeout") {
        TimeoutError::new_err(msg)
    } else if msg.contains("Network") || msg.contains("connection") {
        NetworkError::new_err(msg)
    } else {
        ApiError::new_err(msg)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("PolyoxideError", m.py().get_type::<PolyoxideError>())?;
    m.add("ApiError", m.py().get_type::<ApiError>())?;
    m.add(
        "AuthenticationError",
        m.py().get_type::<AuthenticationError>(),
    )?;
    m.add("ValidationError", m.py().get_type::<ValidationError>())?;
    m.add("RateLimitError", m.py().get_type::<RateLimitError>())?;
    m.add("NetworkError", m.py().get_type::<NetworkError>())?;
    m.add("TimeoutError", m.py().get_type::<TimeoutError>())?;
    Ok(())
}
