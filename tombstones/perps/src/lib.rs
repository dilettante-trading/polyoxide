//! # `polyoxide-perps` has moved
//!
//! This crate is retired. It was the Polymarket Perps market-data client. Since 0.40
//! its code is the `perps` module of
//! [`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind that
//! crate's `perps` feature, and every path that was `polyoxide_perps::…` is
//! `polyoxide_polymarket::perps::…`. Its `ws` feature is `perps-ws`, which turns
//! `perps` on, so `polyoxide_perps::ws` is `polyoxide_polymarket::perps::ws` under
//! `perps-ws`.
//!
//! ```toml
//! [dependencies]
//! polyoxide-polymarket = { version = "0.40", default-features = false, features = ["perps"] }
//! ```
//!
//! The [rename manifest][manifest] maps every moved or removed path to where it went,
//! and the [migration guide][guide] explains the moves.
//!
//! To stay on the old crate, pin `polyoxide-perps = "0.39"`: no release is yanked, and
//! this one only stops the build, with an error that says the same.
//!
//! [manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
//! [guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md

#[cfg(not(docsrs))]
compile_error!(
    "polyoxide-perps is retired: use polyoxide-polymarket with its `perps` feature (`perps-ws` for the old `ws`), at polyoxide_polymarket::perps, as https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md maps every path; or pin polyoxide-perps = \"0.39\""
);
