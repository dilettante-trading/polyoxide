//! # `polyoxide-relay` has moved
//!
//! This crate is retired. It was the client for Polymarket's gasless relayer. Since
//! 0.40 its code is the `relay` module of
//! [`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind that
//! crate's `relay` feature, and every path that was `polyoxide_relay::…` is
//! `polyoxide_polymarket::relay::…`. Its `keychain` feature is `polyoxide-polymarket`'s
//! `keychain`, beside `relay`.
//!
//! ```toml
//! [dependencies]
//! polyoxide-polymarket = { version = "0.40", default-features = false, features = ["relay"] }
//! ```
//!
//! The [rename manifest][manifest] maps every moved or removed path to where it went,
//! and the [migration guide][guide] explains the moves.
//!
//! To stay on the old crate, pin `polyoxide-relay = "0.39"`: no release is yanked, and
//! this one only stops the build, with an error that says the same.
//!
//! [manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
//! [guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md

#[cfg(not(docsrs))]
compile_error!(
    "polyoxide-relay is retired: use polyoxide-polymarket with its `relay` feature, at polyoxide_polymarket::relay, as https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md maps every path; or pin polyoxide-relay = \"0.39\""
);
