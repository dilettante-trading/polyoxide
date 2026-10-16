//! What a crate registers in its `[package.metadata.polyoxide]`, read from its
//! own `Cargo.toml`, so its tests can hold the ids it declares in code to the
//! ones its registration declares.

use std::path::Path;

/// The ids a crate's `[package.metadata.polyoxide]` declares.
///
/// ```
/// use polyoxide_test_support::Registration;
///
/// let declared = Registration::parse(
///     r#"
///     [package]
///     name = "polyoxide-acme"
///
///     [package.metadata.polyoxide]
///     venue = "acme"
///     products = ["spot", "events"]
///     "#,
/// )
/// .unwrap();
/// assert_eq!(declared.venue.as_deref(), Some("acme"));
/// assert_eq!(declared.products, ["spot", "events"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Registration {
    /// `venue`, when the crate declares one.
    pub venue: Option<String>,
    /// `products`, empty when the crate declares none.
    pub products: Vec<String>,
}

impl Registration {
    /// The registration in `<dir>/Cargo.toml`. Panics with the path when the
    /// file cannot be read, is not TOML, or declares a venue or products of
    /// the wrong shape.
    #[track_caller]
    pub fn of(dir: impl AsRef<Path>) -> Self {
        let path = dir.as_ref().join("Cargo.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => panic!("{}: {e}", path.display()),
        };
        match Self::parse(&text) {
            Ok(registration) => registration,
            Err(e) => panic!("{}: {e}", path.display()),
        }
    }

    /// The registration in a manifest's text. A manifest without the table
    /// declares nothing.
    ///
    /// # Errors
    ///
    /// The manifest is not TOML, or its `venue` is not a string or its
    /// `products` not an array of strings.
    pub fn parse(manifest: &str) -> Result<Self, String> {
        let manifest: toml::Table = toml::from_str(manifest).map_err(|e| e.to_string())?;
        let Some(table) = manifest
            .get("package")
            .and_then(|package| package.get("metadata"))
            .and_then(|metadata| metadata.get("polyoxide"))
        else {
            return Ok(Self::default());
        };
        let venue = match table.get("venue") {
            None => None,
            Some(toml::Value::String(venue)) => Some(venue.clone()),
            Some(other) => return Err(format!("`venue` is not a string: {other}")),
        };
        let products = match table.get("products") {
            None => Vec::new(),
            Some(toml::Value::Array(products)) => products
                .iter()
                .map(|product| {
                    product
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("a product is not a string: {product}"))
                })
                .collect::<Result<_, _>>()?,
            Some(other) => return Err(format!("`products` is not an array: {other}")),
        };
        Ok(Self { venue, products })
    }
}

/// The calling crate's registration, read from the `Cargo.toml` beside its
/// sources.
///
/// It has to be a macro: `env!("CARGO_MANIFEST_DIR")` written in this crate
/// would name this crate's directory, so the macro expands it in the calling
/// crate instead.
///
/// ```
/// let declared = polyoxide_test_support::registration!();
/// assert_eq!(declared.venue, None, "test support names no venue");
/// assert!(declared.products.is_empty());
/// ```
#[macro_export]
macro_rules! registration {
    () => {
        $crate::Registration::of(env!("CARGO_MANIFEST_DIR"))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_without_the_table_declares_nothing() {
        let declared = Registration::parse("[package]\nname = \"x\"\n").unwrap();
        assert_eq!(declared, Registration::default());
    }

    #[test]
    fn a_venue_or_products_of_the_wrong_shape_is_refused() {
        for (manifest, fault) in [
            (
                "[package.metadata.polyoxide]\nvenue = 1\n",
                "`venue` is not a string",
            ),
            (
                "[package.metadata.polyoxide]\nproducts = \"spot\"\n",
                "`products` is not an array",
            ),
            (
                "[package.metadata.polyoxide]\nproducts = [\"spot\", 2]\n",
                "a product is not a string",
            ),
        ] {
            let err = Registration::parse(manifest).unwrap_err();
            assert!(err.contains(fault), "{manifest:?}: {err}");
        }
        assert!(Registration::parse("[package\n").is_err(), "not TOML");
    }

    #[test]
    #[should_panic(expected = "Cargo.toml")]
    fn a_missing_manifest_names_its_path() {
        let _ = Registration::of("/no/such/crate");
    }
}
