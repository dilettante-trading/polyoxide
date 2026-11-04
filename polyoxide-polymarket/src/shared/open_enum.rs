//! `polyoxide_venue::open_enum!` with a TypeScript shape (spec D17). Under the
//! `specta` feature, an enum declared through [`specta_open_enum!`] exports as
//! a named union of its known wire spellings and [`OpenString`], so a
//! TypeScript consumer sees `"BUY" | "SELL" | OpenString`, where `OpenString`
//! is `string & {}`: the known literals stay visible, and any other spelling,
//! which `Other(raw)` decodes, still type-checks.

/// Any string beside an open enum's known spellings: `string & {}` in
/// TypeScript (written `(string) & ({ [key in never]: never })`), which keeps
/// the literals of a union with it from collapsing into `string`.
#[cfg(feature = "specta")]
#[derive(specta::Type)]
#[allow(dead_code)] // a TypeScript shape only, never built
pub(crate) struct OpenString {
    #[serde(flatten)]
    text: Text,
    #[serde(flatten)]
    nothing: std::collections::HashMap<Never, Never>,
}

/// `string`, as a field specta can flatten.
#[cfg(feature = "specta")]
struct Text;

#[cfg(feature = "specta")]
impl specta::Type for Text {
    fn inline(type_map: &mut specta::TypeMap, generics: specta::Generics) -> specta::DataType {
        <String as specta::Type>::inline(type_map, generics)
    }
}

#[cfg(feature = "specta")]
impl specta::Flatten for Text {}

/// No value: a map of it is `{ [key in never]: never }`, TypeScript's `{}`.
#[cfg(feature = "specta")]
#[derive(specta::Type)]
enum Never {}

/// Declares an open enum with `polyoxide_venue::open_enum!` and, under
/// `specta`, gives it the named union shape (spec D17): a private `shape`
/// module holds an `#[serde(untagged)]` enum named as the open enum, whose
/// variants are the known spellings (inlined) and [`OpenString`], and the open
/// enum's `Type` delegates to it. The variant list is written once.
macro_rules! specta_open_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $wire:literal ),+ $(,)?
        }
    ) => {
        polyoxide_venue::open_enum! {
            $(#[$meta])*
            $vis enum $name {
                $( $(#[$vmeta])* $variant => $wire ),+
            }
        }

        #[cfg(feature = "specta")]
        const _: () = {
            mod shape {
                #[derive(specta::Type)]
                #[allow(dead_code)]
                pub(super) enum Known {
                    $( #[serde(rename = $wire)] $variant ),+
                }

                #[derive(specta::Type)]
                #[serde(untagged)]
                #[allow(dead_code)]
                pub(super) enum $name {
                    #[specta(inline)]
                    Known(Known),
                    Other($crate::shared::OpenString),
                }
            }

            impl specta::Type for $name {
                fn inline(
                    type_map: &mut specta::TypeMap,
                    generics: specta::Generics,
                ) -> specta::DataType {
                    <shape::$name as specta::Type>::inline(type_map, generics)
                }

                fn reference(
                    type_map: &mut specta::TypeMap,
                    generics: &[specta::DataType],
                ) -> specta::datatype::reference::Reference {
                    <shape::$name as specta::Type>::reference(type_map, generics)
                }
            }
        };
    };
}

pub(crate) use specta_open_enum;
