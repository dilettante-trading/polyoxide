//! CLI subcommand definitions for the `polyoxide` binary, one module per
//! venue: `polyoxide <venue> <module> <verb…>`.

mod common;

pub mod binance;
pub mod completions;
pub mod polymarket;
pub mod stream;

#[cfg(feature = "keychain")]
pub mod credentials;
