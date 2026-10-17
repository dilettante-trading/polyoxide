//! # `polyoxide-clob` has moved
//!
//! This crate is retired. It was the Polymarket CLOB client. Since 0.40 its code is the
//! `clob` module of
//! [`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind that
//! crate's `clob` feature, and every path that was `polyoxide_clob::…` is
//! `polyoxide_polymarket::clob::…`. Its `ws` feature is `clob-ws`, which turns `clob`
//! on, so `polyoxide_clob::ws` is `polyoxide_polymarket::clob::ws` under `clob-ws`. Its
//! `gamma` feature is `polyoxide-polymarket`'s `gamma`, beside `clob`. Its `keychain`
//! feature is `polyoxide-polymarket`'s `keychain`, beside `clob`.
//!
//! ```toml
//! [dependencies]
//! polyoxide-polymarket = { version = "0.40", default-features = false, features = ["clob", "gamma"] }
//! ```
//!
//! `gamma` builds the proxy-wallet lookup through Gamma, which `polyoxide-clob`'s
//! defaults turned on and `clob` alone no longer implies.
//!
//! The [rename manifest][manifest] maps every moved or removed path to where it went,
//! and the [migration guide][guide] explains the moves.
//!
//! To stay on the old crate, pin `polyoxide-clob = "0.39"`: no release is yanked, and
//! this one only stops the build, with an error that says the same.
//!
//! [manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
//! [guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md

#[cfg(not(docsrs))]
compile_error!(
    "polyoxide-clob is retired: use polyoxide-polymarket with its `clob` feature (`clob-ws` for the old `ws`), at polyoxide_polymarket::clob, as https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md maps every path; or pin polyoxide-clob = \"0.39\""
);
