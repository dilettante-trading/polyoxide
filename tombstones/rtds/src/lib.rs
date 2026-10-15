//! # `polyoxide-rtds` has moved
//!
//! This crate is retired. It was the client for Polymarket's RTDS crypto price streams.
//! Since 0.40 its code is the `rtds` module of
//! [`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind that
//! crate's `rtds` feature, and every path that was `polyoxide_rtds::…` is
//! `polyoxide_polymarket::rtds::…`.
//!
//! ```toml
//! [dependencies]
//! polyoxide-polymarket = { version = "0.40", default-features = false, features = ["rtds"] }
//! ```
//!
//! The [rename manifest][manifest] maps every moved or removed path to where it went,
//! and the [migration guide][guide] explains the moves.
//!
//! To stay on the old crate, pin `polyoxide-rtds = "0.39"`: no release is yanked, and
//! this one only stops the build, with an error that says the same.
//!
//! [manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
//! [guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md

#[cfg(not(docsrs))]
compile_error!(
    "polyoxide-rtds is retired: use polyoxide-polymarket with its `rtds` feature, at polyoxide_polymarket::rtds, as https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md maps every path; or pin polyoxide-rtds = \"0.39\""
);
