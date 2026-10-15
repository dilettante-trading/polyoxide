//! OS keychain credential storage.
//!
//! Provides `get`, `set`, and `delete` functions for storing, retrieving, and
//! removing individual credential strings from the platform keychain (macOS
//! Keychain, Windows Credential Manager, Linux Secret Service).
//!
//! A credential of several fields describes itself as a [`StoredCredential`]:
//! its service, its [`Field`]s and which of them are secret, and what makes a
//! value valid. [`store`], [`show`] and [`delete_all`] work on any of them, so
//! each field list exists once, beside the type it loads.
//!
//! Gated behind the `keychain` feature flag.

use keyring::Entry;
use polyoxide_venue::{Class, Classify, Secret};

/// Error type for keychain operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KeychainError {
    /// The requested keychain entry does not exist.
    #[error("Keychain entry not found: service={service}, key={key}")]
    NotFound {
        /// The service name that was queried.
        service: String,
        /// The key name that was queried.
        key: String,
    },

    /// An error from the underlying keychain backend.
    #[error("Keychain error: {0}")]
    Backend(#[from] keyring::Error),

    /// A value [`store`] refused before writing anything. The reason never
    /// quotes the value.
    #[error("Invalid {field} for {service}: {reason}")]
    Invalid {
        /// The service the value was for.
        service: String,
        /// The field the value was for, or the one it needs and lacks.
        field: String,
        /// Why it was refused.
        reason: String,
    },
}

impl KeychainError {
    /// A refusal of `field`'s value for `service`, for a
    /// [`StoredCredential::validate`] to return. `reason` must not quote the
    /// value.
    pub fn invalid(service: &str, field: &str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            service: service.to_owned(),
            field: field.to_owned(),
            reason: reason.into(),
        }
    }
}

/// Every keychain failure is [`Class::InvalidRequest`]: the credential the
/// caller asked for cannot be read on this machine, or the value given cannot
/// be stored, and no venue was asked.
impl Classify for KeychainError {
    fn class(&self) -> Class {
        match self {
            Self::NotFound { .. } | Self::Backend(_) | Self::Invalid { .. } => {
                Class::InvalidRequest
            }
        }
    }
}

/// One field of a [`StoredCredential`]: its key in the keychain, and whether
/// its value is a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The field's key in the keychain, such as `api_key`.
    pub name: &'static str,
    /// Whether the value is a secret, which is never printed back.
    pub secret: bool,
}

impl Field {
    /// A field whose value is a secret, such as a key.
    pub const fn secret(name: &'static str) -> Self {
        Self { name, secret: true }
    }

    /// A field whose value may be shown, such as an address.
    pub const fn plain(name: &'static str) -> Self {
        Self {
            name,
            secret: false,
        }
    }
}

/// A credential kept in the keychain, one entry per field under one service.
///
/// ```
/// use polyoxide_core::keychain::{self, Field, KeychainError, StoredCredential};
/// use polyoxide_venue::Secret;
///
/// struct Example;
///
/// impl StoredCredential for Example {
///     const SERVICE: &'static str = "polyoxide-example";
///     const FIELDS: &'static [Field] = &[Field::secret("api_key"), Field::plain("account")];
///
///     fn validate(values: &[(&str, Secret<String>)]) -> Result<(), KeychainError> {
///         match keychain::given(values, "account") {
///             Some(account) if !account.expose().starts_with("acct-") => Err(
///                 KeychainError::invalid(Self::SERVICE, "account", "does not start with acct-"),
///             ),
///             _ => Ok(()),
///         }
///     }
/// }
///
/// // Refused before anything is written.
/// let err = keychain::store::<Example>(&[("account", Secret::new("x".into()))]).unwrap_err();
/// assert!(matches!(err, KeychainError::Invalid { .. }));
/// ```
pub trait StoredCredential {
    /// The keychain service its fields are stored under.
    const SERVICE: &'static str;

    /// Every field, in the order [`show`] reads them.
    const FIELDS: &'static [Field];

    /// Check `values`, the fields about to be stored, each with its field's
    /// name. [`store`] has already refused a name that is not a field, a name
    /// given twice and an empty value; this adds what the credential itself
    /// needs, such as a key that parses or two fields given together.
    fn validate(values: &[(&str, Secret<String>)]) -> Result<(), KeychainError>;
}

/// The value given for `name` in `values`, if any.
pub fn given<'a>(values: &'a [(&str, Secret<String>)], name: &str) -> Option<&'a Secret<String>> {
    values
        .iter()
        .find(|(given, _)| *given == name)
        .map(|(_, value)| value)
}

/// Store `values` under `C`'s service, each under its field's name.
///
/// Every value is checked before any is written: each must name a field of
/// `C`, once, and none may be empty or whitespace, and then
/// [`C::validate`](StoredCredential::validate) must accept them. A refusal is
/// [`KeychainError::Invalid`]. Fields not in `values` are left as they are.
/// Returns the fields written, in `C`'s order.
pub fn store<C: StoredCredential>(
    values: &[(&str, Secret<String>)],
) -> Result<Vec<Field>, KeychainError> {
    check::<C>(values)?;
    let mut written = Vec::new();
    for field in C::FIELDS {
        if let Some(value) = given(values, field.name) {
            set(C::SERVICE, field.name, value.expose())?;
            written.push(*field);
        }
    }
    Ok(written)
}

/// What [`store`] checks before it writes anything.
fn check<C: StoredCredential>(values: &[(&str, Secret<String>)]) -> Result<(), KeychainError> {
    for (i, (name, value)) in values.iter().enumerate() {
        if !C::FIELDS.iter().any(|field| field.name == *name) {
            return Err(KeychainError::invalid(
                C::SERVICE,
                name,
                "not a field of this credential",
            ));
        }
        if values[..i].iter().any(|(earlier, _)| earlier == name) {
            return Err(KeychainError::invalid(C::SERVICE, name, "given twice"));
        }
        if value.expose().trim().is_empty() {
            return Err(KeychainError::invalid(
                C::SERVICE,
                name,
                "the value is empty",
            ));
        }
    }
    C::validate(values)
}

/// Read every field of `C` from its service, in `C`'s order. Each value is
/// held in a [`Secret`] whatever its field, so printing one is the caller's
/// decision, made by [`Field::secret`](Field#structfield.secret).
pub fn show<C: StoredCredential>() -> Vec<(Field, Result<Secret<String>, KeychainError>)> {
    C::FIELDS
        .iter()
        .map(|field| (*field, get(C::SERVICE, field.name).map(Secret::new)))
        .collect()
}

/// Delete every field of `C` from its service. A field with no entry is not
/// an error.
pub fn delete_all<C: StoredCredential>() -> Result<(), KeychainError> {
    delete_fields(C::SERVICE, C::FIELDS)
}

/// Delete each of `fields` from `service`, as [`delete_all`] does for a
/// [`StoredCredential`]'s own service. A field with no entry is not an error.
pub fn delete_fields(service: &str, fields: &[Field]) -> Result<(), KeychainError> {
    for field in fields {
        delete(service, field.name)?;
    }
    Ok(())
}

/// Retrieve a credential from the OS keychain.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-example"`)
/// * `key` - The key name (e.g., `"api_key"`)
pub fn get(service: &str, key: &str) -> Result<String, KeychainError> {
    let entry = Entry::new(service, key)?;
    entry.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => KeychainError::NotFound {
            service: service.to_string(),
            key: key.to_string(),
        },
        other => KeychainError::Backend(other),
    })
}

/// Store a credential in the OS keychain.
///
/// Creates or overwrites the entry for the given `(service, key)` pair.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-example"`)
/// * `key` - The key name (e.g., `"api_key"`)
/// * `value` - The credential value to store
pub fn set(service: &str, key: &str, value: &str) -> Result<(), KeychainError> {
    let entry = Entry::new(service, key)?;
    entry.set_password(value)?;
    Ok(())
}

/// Delete a credential from the OS keychain.
///
/// Returns `Ok(())` if the entry was deleted or did not exist.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-example"`)
/// * `key` - The key name (e.g., `"api_key"`)
pub fn delete(service: &str, key: &str) -> Result<(), KeychainError> {
    let entry = Entry::new(service, key)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeychainError::Backend(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_error_includes_service_and_key() {
        let err = KeychainError::NotFound {
            service: "test-svc".to_string(),
            key: "test-key".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("test-svc"), "missing service: {msg}");
        assert!(msg.contains("test-key"), "missing key: {msg}");
    }

    #[test]
    fn backend_error_display() {
        let err = KeychainError::Backend(keyring::Error::NoEntry);
        let msg = err.to_string();
        assert!(msg.contains("Keychain error"), "unexpected: {msg}");
    }

    #[test]
    fn every_variant_classifies() {
        let rows = [
            KeychainError::NotFound {
                service: "svc".into(),
                key: "key".into(),
            },
            KeychainError::Backend(keyring::Error::NoEntry),
        ];
        for err in rows {
            assert_eq!(err.class(), Class::InvalidRequest, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
            assert_eq!(err.retry_after(), None, "{err:?}");
            assert!(!err.is_retriable(), "{err:?}");
        }
    }

    /// A credential no test ever writes: every `store` below is refused
    /// before the keyring is touched.
    struct NeverWritten;

    impl StoredCredential for NeverWritten {
        const SERVICE: &'static str = "polyoxide-test-never-written";
        const FIELDS: &'static [Field] = &[Field::secret("key"), Field::plain("label")];

        fn validate(values: &[(&str, Secret<String>)]) -> Result<(), KeychainError> {
            if given(values, "key").is_some() && given(values, "label").is_none() {
                return Err(KeychainError::invalid(
                    Self::SERVICE,
                    "label",
                    "required with key",
                ));
            }
            Err(KeychainError::invalid(
                Self::SERVICE,
                "label",
                "this test credential refuses everything",
            ))
        }
    }

    fn values(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, Secret<String>)> {
        pairs
            .iter()
            .map(|(name, value)| (*name, Secret::new((*value).to_owned())))
            .collect()
    }

    /// The refusal `store` returned, as `(field, reason)`.
    fn refusal(pairs: &[(&'static str, &str)]) -> (String, String) {
        match store::<NeverWritten>(&values(pairs)) {
            Err(KeychainError::Invalid {
                service,
                field,
                reason,
            }) => {
                assert_eq!(service, NeverWritten::SERVICE);
                (field, reason)
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_name_that_is_not_a_field_is_refused() {
        let (field, reason) = refusal(&[("label", "ok"), ("passphrase", "hunter2")]);
        assert_eq!(field, "passphrase");
        assert!(reason.contains("not a field"), "{reason}");
    }

    #[test]
    fn an_empty_or_blank_value_is_refused() {
        for blank in ["", " ", "\t\n"] {
            let (field, reason) = refusal(&[("key", blank), ("label", "ok")]);
            assert_eq!(field, "key", "{blank:?}");
            assert!(reason.contains("empty"), "{reason}");
        }
    }

    #[test]
    fn a_field_given_twice_is_refused() {
        let (field, reason) = refusal(&[("label", "a"), ("label", "b")]);
        assert_eq!(field, "label");
        assert!(reason.contains("twice"), "{reason}");
    }

    #[test]
    fn the_credentials_own_validation_runs_after_the_generic_checks() {
        let (field, reason) = refusal(&[("key", "k")]);
        assert_eq!(field, "label");
        assert_eq!(reason, "required with key");
    }

    #[test]
    fn a_refusal_classifies_as_an_invalid_request_and_never_quotes_the_value() {
        let err = store::<NeverWritten>(&values(&[("key", "hunter2"), ("label", "")])).unwrap_err();
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(err.is_fault());
        assert_eq!(err.retry_after(), None);
        let printed = format!("{err} {err:?}");
        assert!(!printed.contains("hunter2"), "{printed}");
        assert!(
            printed.contains(NeverWritten::SERVICE) && printed.contains("label"),
            "{printed}"
        );
    }

    #[test]
    fn given_finds_a_value_by_its_field_name() {
        let values = values(&[("key", "k"), ("label", "l")]);
        assert_eq!(
            given(&values, "label").map(|v| v.expose().as_str()),
            Some("l")
        );
        assert!(given(&values, "other").is_none());
    }

    #[test]
    fn field_constructors_mark_secrets() {
        assert_eq!(
            Field::secret("key"),
            Field {
                name: "key",
                secret: true
            }
        );
        assert!(!Field::plain("address").secret);
    }
}
