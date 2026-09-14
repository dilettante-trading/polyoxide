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
    with_details(map_api_err(&e), None)
}

/// A Data API v2 error body maps by its stable `code`; anything else keeps the
/// message matching the v1 routes have always used.
pub fn data_err(e: polyoxide_data::DataApiError) -> PyErr {
    use polyoxide_data::{v2::ErrorCode, DataApiError};

    match &e {
        DataApiError::V2(v2) => {
            let msg = e.to_string();
            let err = match v2.code {
                ErrorCode::InvalidRequest => ValidationError::new_err(msg),
                ErrorCode::RateLimited => RateLimitError::new_err(msg),
                ErrorCode::RequestTimeout => TimeoutError::new_err(msg),
                _ => ApiError::new_err(msg),
            };
            with_details(err, Some(v2))
        }
        DataApiError::Pagination(_) => with_details(PolyoxideError::new_err(e.to_string()), None),
        _ => with_details(map_api_err(&e), None),
    }
}

pub fn clob_err(e: polyoxide_clob::ClobError) -> PyErr {
    with_details(map_api_err(&e), None)
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

/// Sets the Data API v2 error fields on the exception: the server's values for
/// a v2 error body and `None` for any other error, so every exception this SDK
/// raises has all six attributes.
fn with_details(err: PyErr, v2: Option<&polyoxide_data::v2::V2Error>) -> PyErr {
    Python::attach(|py| -> PyResult<()> {
        let value = err.value(py);
        value.setattr("status", v2.map(|v| v.status))?;
        value.setattr("code", v2.map(|v| v.code.as_str()))?;
        value.setattr("retryable", v2.map(|v| v.retryable))?;
        value.setattr("trace_id", v2.map(|v| v.trace_id.as_str()))?;
        value.setattr("parameter", v2.and_then(|v| v.parameter.as_deref()))?;
        value.setattr(
            "retry_after",
            v2.and_then(|v| v.retry_after).map(|d| d.as_secs_f64()),
        )?;
        Ok(())
    })
    .expect("an exception instance accepts attributes");
    err
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
