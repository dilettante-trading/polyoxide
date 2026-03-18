use pyo3::prelude::*;
use serde_json::Value;

/// Convert snake_case to camelCase
pub fn snake_to_camel(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Get a field from a serde_json::Value by snake_case name.
/// Tries camelCase first (most common), then snake_case as fallback.
pub fn get_field(py: Python<'_>, value: &Value, field: &str) -> PyResult<Py<PyAny>> {
    let camel = snake_to_camel(field);
    let v = value.get(&camel).or_else(|| value.get(field));
    match v {
        Some(v) => value_to_pyobject(py, v),
        None => Ok(py.None()),
    }
}

/// Convert a serde_json::Value to a Python object using pythonize
pub fn value_to_pyobject(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    Ok(pythonize::pythonize(py, value)?.unbind())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_to_camel() {
        assert_eq!(snake_to_camel("condition_id"), "conditionId");
        assert_eq!(snake_to_camel("id"), "id");
        assert_eq!(snake_to_camel("clob_token_ids"), "clobTokenIds");
        assert_eq!(snake_to_camel("neg_risk"), "negRisk");
    }
}
