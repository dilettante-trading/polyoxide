//! `polyoxide <venue> credentials <store|show|delete> <kind>`: a venue's
//! credentials in the OS keychain.
//!
//! A venue lists its [`Kinds`], each a [`StoredCredential`] the venue's
//! library defines. This module builds the rest for every venue: the three
//! verbs, one flag per field (`api_passphrase` is `--api-passphrase`), the
//! redaction `show` applies, and the keychain calls.

use std::{fmt::Write as _, io::Write, marker::PhantomData};

use clap::{error::ErrorKind, Arg, ArgAction, ArgMatches, Command, FromArgMatches, Subcommand};
use color_eyre::eyre::Result;
use polyoxide_core::{
    keychain::{self, Field, StoredCredential},
    KeychainError,
};
use polyoxide_venue::Secret;

/// The values `store` is given, each under its field's name.
type Values<'a> = [(&'a str, Secret<String>)];

/// What `show` reads: each field, with its value or why there is none.
type Shown = Vec<(Field, Result<Secret<String>, KeychainError>)>;

/// One kind of credential a venue keeps in the keychain, as the CLI serves
/// it. Built by [`Kind::of`], from the type that loads it.
pub struct Kind {
    /// The kind's name on the command line, such as `clob`.
    pub name: &'static str,
    /// One line for `--help`.
    pub about: &'static str,
    /// The keychain service its fields are stored under.
    pub service: &'static str,
    /// Its fields, in order; `store` takes a flag for each.
    pub fields: &'static [Field],
    store: fn(&Values<'_>) -> Result<Vec<Field>, KeychainError>,
    show: fn() -> Shown,
    delete: fn() -> Result<(), KeychainError>,
}

impl Kind {
    /// The kind `name`, stored as `C` is.
    pub const fn of<C: StoredCredential>(name: &'static str, about: &'static str) -> Self {
        Self {
            name,
            about,
            service: C::SERVICE,
            fields: C::FIELDS,
            store: keychain::store::<C>,
            show: keychain::show::<C>,
            delete: keychain::delete_all::<C>,
        }
    }
}

/// A venue's kinds, which its `credentials` command serves.
pub trait Kinds {
    /// Every kind, in the order `--help` lists them.
    const KINDS: &'static [Kind];
}

/// What `credentials` does with its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Write the fields given, and leave the rest as they are.
    Store,
    /// Say which fields are present.
    Show,
    /// Delete every field.
    Delete,
}

/// Each verb: its name, and its line for `--help`.
const VERBS: [(Verb, &str, &str); 3] = [
    (
        Verb::Store,
        "store",
        "Store credentials in the OS keychain; only the fields given are written",
    ),
    (
        Verb::Show,
        "show",
        "Show which credentials are present in the OS keychain",
    ),
    (
        Verb::Delete,
        "delete",
        "Delete every field of a kind from the OS keychain",
    ),
];

/// `credentials <store|show|delete> <kind>` over the venue's kinds `K`.
pub struct Credentials<K> {
    verb: Verb,
    kind: &'static Kind,
    values: Vec<(&'static str, Secret<String>)>,
    venue: PhantomData<K>,
}

impl<K: Kinds> Credentials<K> {
    /// The verb given.
    pub fn verb(&self) -> Verb {
        self.verb
    }

    /// The kind given.
    pub fn kind(&self) -> &'static Kind {
        self.kind
    }

    /// For `store`, each flag given, under its field's name, in the kind's
    /// order.
    pub fn values(&self) -> &[(&'static str, Secret<String>)] {
        &self.values
    }

    pub fn run(self) -> Result<()> {
        let kind = self.kind;
        match self.verb {
            Verb::Store => {
                let mut err = std::io::stderr();
                if self.values.is_empty() {
                    writeln!(
                        err,
                        "No credentials provided. Use --help to see available options."
                    )?;
                    return Ok(());
                }
                // Every value is checked before any is written.
                let written = (kind.store)(&self.values)?;
                writeln!(err, "Stored {} credential(s) in keychain.", written.len())?;
                for field in written {
                    writeln!(err, "  - {}", field.name)?;
                }
            }
            Verb::Show => print!("{}", render(kind.service, &(kind.show)())),
            Verb::Delete => {
                (kind.delete)()?;
                eprintln!(
                    "Deleted every {} credential from the keychain's {}.",
                    kind.name, kind.service
                );
            }
        }
        Ok(())
    }
}

/// `show`'s output: one line per field, `present`, `not found` or
/// `error: …`. A field that is not secret, such as an address, prints its
/// value when present, escaped, since v0.39.0 stored values unchecked and one
/// may carry a newline or a terminal escape; a secret one never prints.
pub fn render(service: &str, shown: &[(Field, Result<Secret<String>, KeychainError>)]) -> String {
    let width = shown
        .iter()
        .map(|(field, _)| field.name.len() + 1)
        .max()
        .unwrap_or(0);
    let mut text = format!("Keychain credentials for {service}:\n");
    for (field, value) in shown {
        let status = match value {
            Ok(value) if !field.secret => value.expose().escape_debug().to_string(),
            Ok(_) => "present".to_owned(),
            Err(KeychainError::NotFound { .. }) => "not found".to_owned(),
            Err(error) => format!("error: {error}"),
        };
        let label = format!("{}:", field.name);
        let _ = writeln!(text, "  {label:<width$}  {status}");
    }
    text
}

/// A field's flag: its name with hyphens, so `api_passphrase` is
/// `--api-passphrase`.
fn flag(field: &Field) -> String {
    field.name.replace('_', "-")
}

/// One verb's command: a subcommand per kind, each with a flag per field
/// when the verb is `store`.
fn verb_command<K: Kinds>(verb: Verb, name: &'static str, about: &'static str) -> Command {
    let mut command = Command::new(name)
        .about(about)
        .subcommand_required(true)
        .arg_required_else_help(true);
    for kind in K::KINDS {
        let mut sub = Command::new(kind.name).about(format!("{} ({})", kind.about, kind.service));
        if verb == Verb::Store {
            for field in kind.fields {
                let help = if field.secret {
                    format!(
                        "The value to store as `{}`; `show` never prints it",
                        field.name
                    )
                } else {
                    format!("The value to store as `{}`", field.name)
                };
                sub = sub.arg(
                    Arg::new(field.name)
                        .long(flag(field))
                        .value_name("VALUE")
                        .action(ArgAction::Set)
                        .value_parser(clap::value_parser!(String))
                        .help(help),
                );
            }
        }
        command = command.subcommand(sub);
    }
    command
}

impl<K: Kinds> FromArgMatches for Credentials<K> {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let (verb_name, verb_matches) = matches
            .subcommand()
            .ok_or_else(|| clap::Error::new(ErrorKind::MissingSubcommand))?;
        let verb = VERBS
            .iter()
            .find(|(_, name, _)| *name == verb_name)
            .map(|(verb, _, _)| *verb)
            .ok_or_else(|| clap::Error::new(ErrorKind::InvalidSubcommand))?;
        let (kind_name, kind_matches) = verb_matches
            .subcommand()
            .ok_or_else(|| clap::Error::new(ErrorKind::MissingSubcommand))?;
        let kind = K::KINDS
            .iter()
            .find(|kind| kind.name == kind_name)
            .ok_or_else(|| clap::Error::new(ErrorKind::InvalidSubcommand))?;
        let values = match verb {
            Verb::Store => kind
                .fields
                .iter()
                .filter_map(|field| {
                    kind_matches
                        .get_one::<String>(field.name)
                        .map(|value| (field.name, Secret::new(value.clone())))
                })
                .collect(),
            Verb::Show | Verb::Delete => Vec::new(),
        };
        Ok(Self {
            verb,
            kind,
            values,
            venue: PhantomData,
        })
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl<K: Kinds> Subcommand for Credentials<K> {
    fn augment_subcommands(command: Command) -> Command {
        VERBS
            .iter()
            .fold(command, |command, (verb, name, about)| {
                command.subcommand(verb_command::<K>(*verb, name, about))
            })
            .subcommand_required(true)
    }

    fn augment_subcommands_for_update(command: Command) -> Command {
        Self::augment_subcommands(command)
    }

    fn has_subcommand(name: &str) -> bool {
        VERBS.iter().any(|(_, verb, _)| *verb == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_prints_a_plain_value_and_never_a_secret_one() {
        let shown = vec![
            (
                Field::secret("api_key"),
                Ok(Secret::new("hunter2-secret-value".to_owned())),
            ),
            (
                Field::plain("relayer_api_key_address"),
                Ok(Secret::new(
                    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_owned(),
                )),
            ),
            (
                Field::secret("passphrase"),
                Err(KeychainError::NotFound {
                    service: "polyoxide-test".into(),
                    key: "passphrase".into(),
                }),
            ),
            (
                Field::secret("api_secret"),
                Err(KeychainError::invalid(
                    "polyoxide-test",
                    "api_secret",
                    "unreadable",
                )),
            ),
        ];
        let text = render("polyoxide-test", &shown);
        assert!(!text.contains("hunter2"), "a secret was printed: {text}");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "Keychain credentials for polyoxide-test:");
        assert!(
            lines[1].contains("api_key:") && lines[1].ends_with(" present"),
            "{text}"
        );
        assert!(
            lines[2].ends_with(" 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            "{text}"
        );
        assert!(lines[3].ends_with(" not found"), "{text}");
        assert!(lines[4].contains(" error: "), "{text}");
    }

    #[test]
    fn show_escapes_a_plain_value_so_it_stays_one_line() {
        let shown = vec![(
            Field::plain("relayer_api_key_address"),
            Ok(Secret::new("0xabc\n\u{1b}[31mforged: present".to_owned())),
        )];
        let text = render("polyoxide-test", &shown);
        assert_eq!(text.lines().count(), 2, "{text}");
        assert!(!text.contains('\u{1b}'), "{text:?}");
        assert!(
            text.lines()
                .nth(1)
                .unwrap()
                .ends_with(" 0xabc\\n\\u{1b}[31mforged: present"),
            "{text}"
        );
    }

    #[test]
    fn a_flag_is_its_field_with_hyphens() {
        assert_eq!(flag(&Field::secret("api_passphrase")), "api-passphrase");
        assert_eq!(
            flag(&Field::plain("relayer_api_key_address")),
            "relayer-api-key-address"
        );
    }
}
