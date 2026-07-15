//! CLI subcommand definitions for the `polyoxide` binary.

mod common;

pub mod clob;
pub mod completions;
pub mod data;
pub mod gamma;
pub mod ws;

pub use clob::ClobCommand;
pub use completions::CompletionsCommand;
pub use data::DataCommand;
pub use gamma::GammaCommand;
pub use ws::WsCommand;

#[cfg(feature = "keychain")]
pub mod credentials;

#[cfg(feature = "keychain")]
pub use credentials::CredentialsCommand;
