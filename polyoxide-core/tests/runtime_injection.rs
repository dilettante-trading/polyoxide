//! The runtime a client is built on, and the plumbing that carries it: from a
//! config or a builder to the client and its `with_base_url` siblings. Tests
//! further down prove that each wait and clock read of core goes through it.

use std::sync::Arc;

use polyoxide_core::{ClientConfig, HttpClientBuilder};
use polyoxide_rt::{Runtime, TokioRuntime};

fn tokio_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}

#[test]
fn a_config_runtime_is_the_clients() {
    let runtime = tokio_runtime();
    let mut config = ClientConfig::new("https://example.test", 4);
    assert!(config.runtime.is_none(), "a new config names no runtime");
    config.runtime = Some(Arc::clone(&runtime));

    let http = config.http_builder().build().unwrap();
    assert!(Arc::ptr_eq(http.runtime(), &runtime));
}

#[test]
fn a_builders_runtime_is_shared_with_its_siblings() {
    let runtime = tokio_runtime();
    let http = HttpClientBuilder::new("https://example.test")
        .runtime(Arc::clone(&runtime))
        .build()
        .unwrap();
    let sibling = http.with_base_url("https://other.example.test").unwrap();

    assert!(Arc::ptr_eq(http.runtime(), &runtime));
    assert!(Arc::ptr_eq(sibling.runtime(), &runtime));
}

#[test]
fn an_unset_runtime_is_the_targets_default() {
    // Native targets default to tokio's runtime.
    let http = HttpClientBuilder::new("https://example.test")
        .build()
        .unwrap();
    assert_eq!(format!("{:?}", http.runtime()), "TokioRuntime");

    let config = ClientConfig::new("https://example.test", 4);
    assert_eq!(format!("{:?}", config.runtime_or_default()), "TokioRuntime");
}

struct Knobs {
    config: ClientConfig,
}

// Only `runtime` is read here; the other setters are the macro's.
#[allow(dead_code)]
impl Knobs {
    polyoxide_core::client_config_setters!(config);
}

#[test]
fn the_runtime_setter_writes_the_config() {
    let runtime = tokio_runtime();
    let knobs = Knobs {
        config: ClientConfig::new("https://example.test", 4),
    }
    .runtime(Arc::clone(&runtime));

    let set = knobs
        .config
        .runtime
        .as_ref()
        .expect("the setter wrote the config");
    assert!(Arc::ptr_eq(set, &runtime));
    assert!(Arc::ptr_eq(&knobs.config.runtime_or_default(), &runtime));
}
