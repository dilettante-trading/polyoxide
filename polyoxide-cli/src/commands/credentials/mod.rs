//! Keychain credential management commands.

use clap::{Args, Subcommand, ValueEnum};
use color_eyre::eyre::Result;

#[derive(Args)]
pub struct CredentialsCommand {
    #[command(subcommand)]
    command: CredentialsSubcommand,
}

#[derive(Subcommand)]
enum CredentialsSubcommand {
    /// Store credentials in the OS keychain
    Store {
        #[command(subcommand)]
        target: StoreTarget,
    },
    /// Show which credentials are present in the OS keychain
    Show {
        /// Which service to check
        #[arg(value_enum)]
        target: ShowTarget,
    },
    /// Delete credentials from the OS keychain
    Delete {
        /// Which service to delete credentials for
        #[arg(value_enum)]
        target: DeleteTarget,
    },
}

#[derive(Subcommand)]
enum StoreTarget {
    /// Store CLOB API credentials
    Clob(StoreClobArgs),
    /// Store Relay API credentials
    Relay(StoreRelayArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum ShowTarget {
    /// Check CLOB credentials
    Clob,
    /// Check Relay credentials
    Relay,
}

#[derive(Clone, Copy, ValueEnum)]
enum DeleteTarget {
    /// Delete CLOB credentials
    Clob,
    /// Delete Relay credentials
    Relay,
}

#[derive(Args)]
struct StoreClobArgs {
    /// Hex-encoded private key
    #[arg(long)]
    private_key: Option<String>,
    /// API key
    #[arg(long)]
    api_key: Option<String>,
    /// API secret (base64 encoded)
    #[arg(long)]
    api_secret: Option<String>,
    /// API passphrase
    #[arg(long)]
    api_passphrase: Option<String>,
}

#[derive(Args)]
struct StoreRelayArgs {
    /// Hex-encoded private key
    #[arg(long)]
    private_key: Option<String>,
    /// Builder API key
    #[arg(long)]
    api_key: Option<String>,
    /// Builder API secret
    #[arg(long)]
    api_secret: Option<String>,
    /// Builder API passphrase
    #[arg(long)]
    passphrase: Option<String>,
    /// Relayer API key (alternative to builder credentials)
    #[arg(long)]
    relayer_api_key: Option<String>,
    /// Relayer API key address
    #[arg(long)]
    relayer_api_key_address: Option<String>,
}

impl CredentialsCommand {
    pub fn run(self) -> Result<()> {
        match self.command {
            CredentialsSubcommand::Store { target } => match target {
                StoreTarget::Clob(args) => store_clob(args),
                StoreTarget::Relay(args) => store_relay(args),
            },
            CredentialsSubcommand::Show { target } => match target {
                ShowTarget::Clob => show_clob(),
                ShowTarget::Relay => show_relay(),
            },
            CredentialsSubcommand::Delete { target } => match target {
                DeleteTarget::Clob => delete_clob(),
                DeleteTarget::Relay => delete_relay(),
            },
        }
    }
}

fn store_clob(args: StoreClobArgs) -> Result<()> {
    use polyoxide_core::keychain;
    let service = polyoxide_clob::KEYCHAIN_SERVICE;

    let mut stored = Vec::new();

    if let Some(val) = &args.private_key {
        keychain::set(service, "private_key", val)?;
        stored.push("private_key");
    }
    if let Some(val) = &args.api_key {
        keychain::set(service, "api_key", val)?;
        stored.push("api_key");
    }
    if let Some(val) = &args.api_secret {
        keychain::set(service, "api_secret", val)?;
        stored.push("api_secret");
    }
    if let Some(val) = &args.api_passphrase {
        keychain::set(service, "api_passphrase", val)?;
        stored.push("api_passphrase");
    }

    if stored.is_empty() {
        eprintln!("No credentials provided. Use --help to see available options.");
    } else {
        eprintln!("Stored {} credential(s) in keychain.", stored.len());
        for key in &stored {
            eprintln!("  - {key}");
        }
    }

    Ok(())
}

fn store_relay(args: StoreRelayArgs) -> Result<()> {
    use polyoxide_core::keychain;
    let service = polyoxide_relay::KEYCHAIN_SERVICE;

    let mut stored = Vec::new();

    if let Some(val) = &args.private_key {
        keychain::set(service, "private_key", val)?;
        stored.push("private_key");
    }
    if let Some(val) = &args.api_key {
        keychain::set(service, "api_key", val)?;
        stored.push("api_key");
    }
    if let Some(val) = &args.api_secret {
        keychain::set(service, "api_secret", val)?;
        stored.push("api_secret");
    }
    if let Some(val) = &args.passphrase {
        keychain::set(service, "passphrase", val)?;
        stored.push("passphrase");
    }
    if let Some(val) = &args.relayer_api_key {
        keychain::set(service, "relayer_api_key", val)?;
        stored.push("relayer_api_key");
    }
    if let Some(val) = &args.relayer_api_key_address {
        keychain::set(service, "relayer_api_key_address", val)?;
        stored.push("relayer_api_key_address");
    }

    if stored.is_empty() {
        eprintln!("No credentials provided. Use --help to see available options.");
    } else {
        eprintln!("Stored {} credential(s) in keychain.", stored.len());
        for key in &stored {
            eprintln!("  - {key}");
        }
    }

    Ok(())
}

/// Check whether a keychain entry exists without leaking the value.
///
/// Note: the `keyring` crate has no "exists?" API, so this reads the full value
/// and immediately discards it.
fn check_entry(service: &str, key: &str) -> &'static str {
    match polyoxide_core::keychain::get(service, key) {
        Ok(_) => "present",
        Err(polyoxide_core::KeychainError::NotFound { .. }) => "not found",
        Err(_) => "error",
    }
}

fn show_clob() -> Result<()> {
    let service = polyoxide_clob::KEYCHAIN_SERVICE;

    println!("Keychain credentials for {service}:");
    println!("  private_key:     {}", check_entry(service, "private_key"));
    println!("  api_key:         {}", check_entry(service, "api_key"));
    println!("  api_secret:      {}", check_entry(service, "api_secret"));
    println!(
        "  api_passphrase:  {}",
        check_entry(service, "api_passphrase")
    );
    Ok(())
}

fn show_relay() -> Result<()> {
    let service = polyoxide_relay::KEYCHAIN_SERVICE;

    println!("Keychain credentials for {service}:");
    println!(
        "  private_key:              {}",
        check_entry(service, "private_key")
    );
    println!(
        "  api_key:                  {}",
        check_entry(service, "api_key")
    );
    println!(
        "  api_secret:               {}",
        check_entry(service, "api_secret")
    );
    println!(
        "  passphrase:               {}",
        check_entry(service, "passphrase")
    );
    println!(
        "  relayer_api_key:          {}",
        check_entry(service, "relayer_api_key")
    );
    println!(
        "  relayer_api_key_address:  {}",
        check_entry(service, "relayer_api_key_address")
    );
    Ok(())
}

fn delete_clob() -> Result<()> {
    use polyoxide_core::keychain;
    let service = polyoxide_clob::KEYCHAIN_SERVICE;

    for key in ["private_key", "api_key", "api_secret", "api_passphrase"] {
        keychain::delete(service, key)?;
    }
    eprintln!("Deleted all CLOB credentials from keychain.");
    Ok(())
}

fn delete_relay() -> Result<()> {
    use polyoxide_core::keychain;
    let service = polyoxide_relay::KEYCHAIN_SERVICE;

    for key in [
        "private_key",
        "api_key",
        "api_secret",
        "passphrase",
        "relayer_api_key",
        "relayer_api_key_address",
    ] {
        keychain::delete(service, key)?;
    }
    eprintln!("Deleted all Relay credentials from keychain.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        cmd: CredentialsCommand,
    }

    fn try_parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_from(args)
    }

    #[test]
    fn store_clob_parses_all_flags() {
        let cli = try_parse(&[
            "test",
            "store",
            "clob",
            "--private-key",
            "0xabc",
            "--api-key",
            "k",
            "--api-secret",
            "s",
            "--api-passphrase",
            "p",
        ])
        .unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store {
                target: StoreTarget::Clob(_)
            }
        ));
    }

    #[test]
    fn store_relay_parses_builder_flags() {
        let cli = try_parse(&[
            "test",
            "store",
            "relay",
            "--private-key",
            "0xabc",
            "--api-key",
            "k",
            "--api-secret",
            "s",
        ])
        .unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store {
                target: StoreTarget::Relay(_)
            }
        ));
    }

    #[test]
    fn store_relay_parses_relayer_api_key_flags() {
        let cli = try_parse(&[
            "test",
            "store",
            "relay",
            "--private-key",
            "0xabc",
            "--relayer-api-key",
            "rk",
            "--relayer-api-key-address",
            "0xaddr",
        ])
        .unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store {
                target: StoreTarget::Relay(_)
            }
        ));
    }

    #[test]
    fn show_clob_parses() {
        let cli = try_parse(&["test", "show", "clob"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Show {
                target: ShowTarget::Clob
            }
        ));
    }

    #[test]
    fn show_relay_parses() {
        let cli = try_parse(&["test", "show", "relay"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Show {
                target: ShowTarget::Relay
            }
        ));
    }

    #[test]
    fn store_clob_no_flags_parses() {
        // All flags are optional — should parse even with none
        let cli = try_parse(&["test", "store", "clob"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Store {
                target: StoreTarget::Clob(_)
            }
        ));
    }

    #[test]
    fn delete_clob_parses() {
        let cli = try_parse(&["test", "delete", "clob"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Delete {
                target: DeleteTarget::Clob
            }
        ));
    }

    #[test]
    fn delete_relay_parses() {
        let cli = try_parse(&["test", "delete", "relay"]).unwrap();
        assert!(matches!(
            cli.cmd.command,
            CredentialsSubcommand::Delete {
                target: DeleteTarget::Relay
            }
        ));
    }
}
