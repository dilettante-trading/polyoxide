/// Macro to generate both async and sync namespace structs from a single definition.
///
/// The body of each method can use `client` (an `Arc<ClientType>`) and `.await`.
/// The macro wraps the body in `future_into_py` for async and `block_on` for sync.
///
/// Due to macro hygiene, the local variable name must come from the call site.
/// Pass `client_var = client` so the body's `client` references match.
macro_rules! client_ns {
    (
        async_name = $async_name:ident,
        sync_name = $sync_name:ident,
        py_async_name = $py_async_name:literal,
        py_sync_name = $py_sync_name:literal,
        client_type = $client_type:ty,
        client_var = $cvar:ident,
        $(
            $(#[$mattr:meta])*
            fn $method:ident ($($param:ident : $ptype:ty),* $(,)?) -> $ret:ty
            { $($body:tt)* }
        )*
    ) => {
        #[pyo3::pyclass(name = $py_async_name, skip_from_py_object)]
        pub struct $async_name {
            pub(crate) client: std::sync::Arc<$client_type>,
        }

        #[pyo3::pymethods]
        impl $async_name {
            $(
                $(#[$mattr])*
                fn $method<'py>(
                    &self,
                    py: pyo3::Python<'py>,
                    $($param: $ptype,)*
                ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::PyAny>> {
                    let $cvar = self.client.clone();
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        $($body)*
                    })
                }
            )*
        }

        #[pyo3::pyclass(name = $py_sync_name, skip_from_py_object)]
        pub struct $sync_name {
            pub(crate) client: std::sync::Arc<$client_type>,
        }

        #[pyo3::pymethods]
        impl $sync_name {
            $(
                $(#[$mattr])*
                fn $method(
                    &self,
                    py: pyo3::Python<'_>,
                    $($param: $ptype,)*
                ) -> pyo3::PyResult<$ret> {
                    let $cvar = self.client.clone();
                    py.detach(|| {
                        crate::runtime::runtime().block_on(async move {
                            $($body)*
                        })
                    })
                }
            )*
        }
    };
}

/// Parse a Python string into a Rust enum variant.
macro_rules! parse_enum {
    ($s:expr, $ty:ty, $($variant:ident => $str:literal),+ $(,)?) => {
        match $s.to_uppercase().as_str() {
            $($str => Ok(<$ty>::$variant),)+
            _ => Err(pyo3::exceptions::PyValueError::new_err(
                format!("invalid value '{}', expected one of: {}", $s, [$($str),+].join(", "))
            ))
        }
    };
}

pub mod gamma;
pub mod data;
pub mod clob;

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    gamma::register(m)?;
    data::register(m)?;
    clob::register(m)?;
    Ok(())
}
