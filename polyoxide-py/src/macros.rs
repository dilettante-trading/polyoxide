/// Generate a Python wrapper type from a serde-serializable Rust type.
///
/// Fields can use `snake_name` (auto camelCase lookup) or `snake_name => "exactKey"` for
/// custom serde renames.
///
/// Usage:
///   py_type!(PyMarket, "Market", polyoxide_gamma::types::Market,
///       id, condition_id, question,
///       question_id => "questionID",
///   );
macro_rules! py_type {
    ($py_name:ident, $py_str:literal, $rust_type:ty,
        $( $field:ident $( => $key:literal )? ),* $(,)?
    ) => {
        #[pyo3::pyclass(name = $py_str, skip_from_py_object)]
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
                    py_type!(@resolve_field py, &self.inner, $field $(, $key)?)
                }
            )*
        }
    };

    // Internal: field with explicit JSON key
    (@resolve_field $py:ident, $val:expr, $field:ident, $key:literal) => {
        $crate::convert::get_field_exact($py, $val, $key)
    };
    // Internal: field with auto snake→camel lookup
    (@resolve_field $py:ident, $val:expr, $field:ident) => {
        $crate::convert::get_field($py, $val, stringify!($field))
    };
}
