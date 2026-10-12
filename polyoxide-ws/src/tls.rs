//! The process-default rustls `CryptoProvider` (W1).

/// Make sure rustls has a default `CryptoProvider` before a connection opens.
///
/// `tokio-tungstenite` builds its TLS config from the process-wide default
/// provider, and rustls installs one by itself only when exactly one backend
/// feature is enabled. `reqwest` (through `polyoxide-core`) enables `ring` and
/// `alloy`'s `reqwest 0.13` enables `aws-lc-rs` on the same `rustls`, so a
/// consumer with both in its graph gets neither, and `connect_async` panics.
///
/// Call it before any `tokio_tungstenite` connect. The kit's own connect calls
/// it, and so should a live test or a caller that connects by hand. It installs
/// `ring`'s provider once per process, and only when none is installed:
/// `install_default` refuses to replace one, so the host application's own
/// choice, or another crate's install racing this one, is left alone.
///
/// The basis is one socket crate's former copy (`src/client.rs`),
/// byte-identical to the copies the four other socket crates carried, each
/// deleted for this one (Stories 4.6 to 4.9). No per-venue divergence row
/// applies.
pub fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crypto_provider_is_installed_before_any_connection_is_opened() {
        // `connect_async` builds its TLS config from the process-wide default
        // provider and panics if there is none. Two `rustls` backend features
        // are enabled in this workspace, so rustls installs neither by itself
        // — see the comment on `ensure_crypto_provider`.
        //
        // What this proves is narrow but real: that calling the installer
        // leaves a provider in place. It cannot prove ordering, since any
        // earlier connect in this binary already ran it. It fails if the
        // `ring` feature is dropped or `install_default` stops working, which
        // is the regression that would otherwise only surface as a panic in
        // the nightly live run.
        ensure_crypto_provider();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "no default CryptoProvider; connect_async would panic"
        );
    }

    #[test]
    fn a_default_crypto_provider_is_available_to_connect() {
        // Moved from one socket crate (`ws/client.rs`), whose copy of the
        // install it pinned, by name (Story 4.9). Every `connect_*` of that
        // crate panicked before its install existed: tokio-tungstenite builds
        // its rustls `ClientConfig` from the process-wide default
        // CryptoProvider, and that crate's graph enables *both* `ring` (via
        // reqwest 0.12) and `aws-lc-rs` (via alloy's reqwest 0.13) on one
        // shared rustls. Given two candidates rustls installs neither and
        // panics, so the whole WebSocket surface aborted at connect time.
        ensure_crypto_provider();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "no process-default CryptoProvider: every WebSocket connect will panic"
        );
    }
}
