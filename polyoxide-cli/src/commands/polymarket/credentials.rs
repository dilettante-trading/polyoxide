//! Polymarket's credential kinds, for `polyoxide polymarket credentials`.

use polyoxide_polymarket::{clob::Account, relay::BuilderAccount};

use crate::commands::credentials::{Kind, Kinds};

/// Polymarket's two kinds: `clob` and `builder`.
pub struct Polymarket;

impl Kinds for Polymarket {
    const KINDS: &'static [Kind] = &[
        Kind::of::<Account>("clob", "CLOB API credentials and the signing key"),
        Kind::of::<BuilderAccount>(
            "builder",
            "The relay's builder API credentials or relayer API key, and the signing key",
        ),
    ];
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use polyoxide_core::{
        keychain::{Field, StoredCredential},
        KeychainError,
    };
    use polyoxide_venue::Secret;

    use super::*;
    use crate::commands::credentials::{render, Credentials, Verb};

    #[derive(Parser)]
    struct Wrapper {
        #[command(subcommand)]
        cmd: Credentials<Polymarket>,
    }

    fn parse(argv: &[&str]) -> Credentials<Polymarket> {
        Wrapper::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("{argv:?}: {e}"))
            .cmd
    }

    /// Each value `store` was given, as `(field, value)`.
    fn given(cmd: &Credentials<Polymarket>) -> Vec<(&str, &str)> {
        cmd.values()
            .iter()
            .map(|(name, value)| (*name, value.expose().as_str()))
            .collect()
    }

    #[test]
    fn the_kinds_keep_v0_39s_services() {
        let services: Vec<(&str, &str)> = Polymarket::KINDS
            .iter()
            .map(|kind| (kind.name, kind.service))
            .collect();
        assert_eq!(
            services,
            [("clob", "polyoxide-clob"), ("builder", "polyoxide-relay")]
        );
    }

    #[test]
    fn every_verb_parses_with_every_kind() {
        for (verb, name) in [
            (Verb::Store, "store"),
            (Verb::Show, "show"),
            (Verb::Delete, "delete"),
        ] {
            for kind in ["clob", "builder"] {
                let cmd = parse(&["test", name, kind]);
                assert_eq!(cmd.verb(), verb, "{name} {kind}");
                assert_eq!(cmd.kind().name, kind, "{name} {kind}");
                assert!(cmd.values().is_empty(), "{name} {kind}");
            }
        }
    }

    #[test]
    fn a_verb_needs_a_kind_and_relay_is_now_builder() {
        for name in ["store", "show", "delete"] {
            assert!(Wrapper::try_parse_from(["test", name]).is_err(), "{name}");
            assert!(
                Wrapper::try_parse_from(["test", name, "relay"]).is_err(),
                "{name} relay"
            );
        }
        assert!(Wrapper::try_parse_from(["test"]).is_err());
    }

    #[test]
    fn store_clob_takes_v0_39s_flags() {
        let cmd = parse(&[
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
        ]);
        assert_eq!(
            given(&cmd),
            [
                ("private_key", "0xabc"),
                ("api_key", "k"),
                ("api_secret", "s"),
                ("api_passphrase", "p"),
            ]
        );
    }

    #[test]
    fn store_builder_takes_v0_39s_relay_flags() {
        let cmd = parse(&[
            "test",
            "store",
            "builder",
            "--private-key",
            "0xabc",
            "--api-key",
            "k",
            "--api-secret",
            "s",
            "--passphrase",
            "p",
            "--relayer-api-key",
            "rk",
            "--relayer-api-key-address",
            "0xaddr",
        ]);
        assert_eq!(
            given(&cmd),
            [
                ("private_key", "0xabc"),
                ("api_key", "k"),
                ("api_secret", "s"),
                ("passphrase", "p"),
                ("relayer_api_key", "rk"),
                ("relayer_api_key_address", "0xaddr"),
            ]
        );
    }

    #[test]
    fn store_takes_only_the_fields_given_and_show_and_delete_take_none() {
        let cmd = parse(&["test", "store", "builder", "--api-key", "k"]);
        assert_eq!(given(&cmd), [("api_key", "k")]);
        for name in ["show", "delete"] {
            assert!(
                Wrapper::try_parse_from(["test", name, "clob", "--api-key", "k"]).is_err(),
                "{name} took a field flag"
            );
        }
        // A flag of the other kind is refused, not ignored.
        assert!(Wrapper::try_parse_from(["test", "store", "clob", "--passphrase", "p"]).is_err());
    }

    /// `BuilderAccount`'s fields and validation on a service no developer
    /// stores to, so a store that got past validation could not reach the real
    /// `polyoxide-relay` entries.
    struct IsolatedBuilder;

    impl StoredCredential for IsolatedBuilder {
        const SERVICE: &'static str = "polyoxide-cli-test-isolated-builder";
        const FIELDS: &'static [Field] = BuilderAccount::FIELDS;

        fn validate(values: &[(&str, Secret<String>)]) -> Result<(), KeychainError> {
            BuilderAccount::validate(values)
        }
    }

    /// Polymarket's `builder` kind, on [`IsolatedBuilder`]'s service.
    struct Isolated;

    impl Kinds for Isolated {
        const KINDS: &'static [Kind] = &[Kind::of::<IsolatedBuilder>("builder", "isolated")];
    }

    #[derive(Parser)]
    struct IsolatedWrapper {
        #[command(subcommand)]
        cmd: Credentials<Isolated>,
    }

    #[test]
    fn a_relayer_key_without_its_address_is_refused_before_anything_is_written() {
        let argv = ["test", "store", "builder", "--relayer-api-key", "k"];
        // The real command takes this argv; the isolated twin, with the same
        // fields, runs it.
        assert_eq!(given(&parse(&argv)), [("relayer_api_key", "k")]);
        let cmd = IsolatedWrapper::try_parse_from(argv).unwrap().cmd;
        assert_eq!(cmd.kind().service, IsolatedBuilder::SERVICE);
        let err = cmd.run().unwrap_err();
        match err.downcast_ref::<KeychainError>() {
            Some(KeychainError::Invalid { service, field, .. }) => {
                assert_eq!(service, "polyoxide-relay");
                assert_eq!(field, "relayer_api_key_address");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn show_builder_prints_the_address_and_hides_every_secret() {
        let builder = &Polymarket::KINDS[1];
        let shown: Vec<_> = builder
            .fields
            .iter()
            .map(|field| {
                let value = if field.secret {
                    format!("secret-{}", field.name)
                } else {
                    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_owned()
                };
                (*field, Ok(Secret::new(value)))
            })
            .collect();
        let text = render(builder.service, &shown);
        assert!(!text.contains("secret-"), "a secret was printed: {text}");
        assert_eq!(text.matches(" present").count(), 5, "{text}");
        assert!(
            text.contains("relayer_api_key_address:")
                && text.contains("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            "{text}"
        );
    }
}
