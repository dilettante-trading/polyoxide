/// Generate a Python wrapper type from a serde-serializable Rust type.
///
/// Usage:
///   py_type!(PyMarket, "Market", polyoxide_gamma::types::Market,
///       id, condition_id, question, description, slug, active, closed
///   );
macro_rules! py_type {
    ($py_name:ident, $py_str:literal, $rust_type:ty, $( $field:ident ),* $(,)?) => {
        #[pyo3::pyclass(name = $py_str)]
        #[derive(Clone)]
        pub struct $py_name {
            inner: serde_json::Value,
        }

        impl From<$rust_type> for $py_name {
            fn from(val: $rust_type) -> Self {
                Self {
                    inner: serde_json::to_value(val)
                        .expect("serialization of known type cannot fail"),
                }
            }
        }

        impl $py_name {
            pub fn inner(&self) -> &serde_json::Value {
                &self.inner
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            pub fn to_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                $crate::convert::value_to_pyobject(py, &self.inner)
            }

            pub fn __repr__(&self) -> String {
                format!("{}({})", $py_str, &self.inner)
            }

            pub fn __str__(&self) -> String {
                self.inner.to_string()
            }

            $(
                #[getter]
                pub fn $field(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    $crate::convert::get_field(py, &self.inner, stringify!($field))
                }
            )*
        }
    };
}

pub(crate) use py_type;
