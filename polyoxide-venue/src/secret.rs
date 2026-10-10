//! A value that never prints.

use std::fmt;

/// A credential or other value that must not reach a log.
///
/// Its `Debug` prints `Secret([REDACTED])` whatever it holds, and it has no
/// `Display`, so a struct that derives `Debug` can carry one safely. Read the
/// value with [`expose`](Self::expose), at the one place that needs it.
///
/// ```
/// use polyoxide_venue::Secret;
///
/// let key = Secret::new(String::from("hunter2"));
/// assert_eq!(format!("{key:?}"), "Secret([REDACTED])");
/// assert_eq!(key.expose(), "hunter2");
/// ```
#[derive(Clone)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wraps a value.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// The value. Every read goes through here, so each one is easy to find.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    #[allow(dead_code)]
    struct Credentials {
        key: &'static str,
        secret: Secret<&'static str>,
    }

    #[test]
    fn debug_never_shows_the_value() {
        let creds = Credentials {
            key: "public",
            secret: Secret::new("hunter2"),
        };
        let printed = format!("{creds:?} {creds:#?}");
        assert!(!printed.contains("hunter2"), "{printed}");
        assert!(printed.contains("public"), "{printed}");
        assert!(printed.contains("Secret([REDACTED])"), "{printed}");
    }

    #[test]
    fn a_clone_holds_the_same_value() {
        let secret = Secret::new(String::from("hunter2"));
        assert_eq!(secret.clone().expose(), secret.expose());
    }
}
